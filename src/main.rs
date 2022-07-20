use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, Stdio};

macro_rules! tty_af {
    ($num:literal) => { concat!("\x1b[", $num, "m") };
}

const TTY_CLEAR: &str = tty_af!(0);
const TTY_BOLD: &str = tty_af!(1);
const TTY_RED: &str = tty_af!(31);
const TTY_BLUE: &str = tty_af!(34);

fn stash_ref(id: u32) -> String {
    format!("stash@{{{}}}", id)
}

fn git<I, S>(args: I) -> Command
where I: IntoIterator<Item = S>,
      S: AsRef<OsStr>
{
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd
}

fn has_local_changes() -> io::Result<bool> {
    let has = !git(["status", "--porcelain"])
        .output()?
        .stdout
        .is_empty();
    Ok(has)
}

fn error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, message)
}

fn git_stash_show(stash_num: u32) -> io::Result<bool> {
    let code = git(["stash", "show", "-p", &stash_ref(stash_num)])
        .stderr(Stdio::null())
        .status()?
        .code()
        .ok_or_else(|| error("terminated by signal"))?;
    Ok(code == 0 || code == 141)
}

fn git_stashes_is_empty() -> io::Result<bool> {
    git(["rev-parse", "-q", "--verify", "refs/stash"])
        .status()
        .map(|s| !s.success())
}

fn read_line() -> io::Result<String> {
    io::stdin().lock().lines().next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?
}

fn drop_stash(stash_num: u32) -> io::Result<()> {
    let stash_name = stash_ref(stash_num);
    let applied = !git(["stash-applied", &stash_name])
        .status()?
        .success();
    if !applied {
        print!("Stash may not be applied. Drop anyway? [y/N] ");
        io::stdout().flush()?;
        if read_line()? != "y" {
            return Ok(());
        }
    }
    git(["stash", "drop", &stash_name]).status()?;
    Ok(())
}

fn commit_to_branch(stash_num: u32, can_save_branch: bool) -> io::Result<()> {
    let stash_name = stash_ref(stash_num);
    if !can_save_branch {
        eprintln!(
            "{TTY_BOLD}{TTY_RED}\
            ERROR - Can't commit branches with unstaged files!.\
            {TTY_CLEAR}"
        );
        return Ok(());
    }
    let commit_msg_file = ".git/COMMIT_EDITMSG";

    let branch_name = "stash/__TEMP_STASH__";
    git(["checkout", "-b", branch_name]).status()?;
    git(["stash", "apply", &stash_name]).status()?;
    git(["add", "."]).status()?;
    if !git(["commit", "-n"]).status()?.success() {
        git(["reset", "HEAD"]).status()?;
        git(["checkout", "."]).status()?;
        git(["clean", "-f"]).status()?;
        git(["checkout", "-"]).status()?;
        git(["branch", "-d", branch_name]).status()?;
        return Ok(());
    }

    // Change the branch name to the first line of the commit message.
    let file = File::open(commit_msg_file)?;
    let reader = BufReader::new(file);
    let subject = reader.lines()
        .map_while(|line| line.ok())
        .find_map(|line| {
            (!line.is_empty() && !line.starts_with('#')).then(|| line)
        })
        .ok_or_else(|| error("no lines found"))?;

    let subject_terms: Vec<_> = subject.trim().split_whitespace().collect();
    let mut subject = subject_terms.join("_");
    subject.retain(|c| c == '_' || c.is_alphanumeric());
    let new_branch_name = format!("stash/{}", &subject.to_lowercase());

    git(["branch", "-m", &new_branch_name]).status()?;
    git(["checkout", "-"]).status()?;
    git(["stash", "drop", &stash_name]).status()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let can_save_branch = !has_local_changes()?;
    if !can_save_branch {
        eprintln!(
            "{TTY_BOLD}{TTY_RED}\
            WARNING - Can't backup stashes as branches with local changes.\n\
            Resolve local changes to backup stashes as branches.\
            {TTY_CLEAR}"
        );
    }
    let mut stash_num = 0;
    if git_stashes_is_empty()? {
        println!("No stashes found.");
        return Ok(());
    }
    while git_stash_show(stash_num)? {
        print!("{TTY_BOLD}{TTY_BLUE}Action on this stash [d,b,s,a,q,?]? {TTY_CLEAR}");
        io::stdout().flush()?;
        let action = match read_line() {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                println!();
                break;
            }
            result => result?,
        };
        match action.as_str() {
            "d" => match drop_stash(stash_num) {
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    println!();
                    break;
                }
                result => { result?; }
            }
            "b" => commit_to_branch(stash_num, can_save_branch)?,
            "s" => { stash_num += 1; }
            "a" => {
                git(["stash", "apply", &stash_ref(stash_num)]);
                break;
            }
            "q" => { break; }
            "?" | "" => {
                println!(
                    "{TTY_BOLD}{TTY_RED}\
                    d - drop this stash\n\
                    b - commit this stash to a separate branch and delete it\n\
                    s - take no action on this stash\n\
                    a - apply; apply the stash and take no further action\n\
                    q - quit; take no further action on remaining stashes\n\
                    ? - print help\
                    {TTY_CLEAR}"
                );
            }
            _ => (),
        }
    }
    Ok(())
}
