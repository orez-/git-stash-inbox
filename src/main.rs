use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process::Command;
use const_format::formatcp;

macro_rules! tty_af {
    ($num:literal) => { concat!("\x1b[", $num, "m") };
}

const TTY_CLEAR: &str = tty_af!(0);
const TTY_BOLD: &str = tty_af!(1);
const TTY_RED: &str = tty_af!(31);
const TTY_BLUE: &str = tty_af!(34);
const TTY_BOLD_RED: &str = formatcp!("{}{}", TTY_BOLD, TTY_RED);
const TTY_BOLD_BLUE: &str = formatcp!("{}{}", TTY_BOLD, TTY_BLUE);

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

fn has_local_changes() -> bool {
    !git(["status", "--porcelain"])
        .output()
        .unwrap()
        .stdout
        .is_empty()
}

fn git_stash_show(stash_num: u32) -> bool {
    let code = git(["stash", "show", "-p", &stash_ref(stash_num)])
        // TODO: redirect stderr=fnull
        .status()
        .unwrap()
        .code()
        .unwrap();
    code != 0 && code != 141
}

fn read_line() -> String {
    io::stdin().lock().lines().next().unwrap().unwrap()
}

fn drop_stash(stash_num: u32) {
    let stash_name = stash_ref(stash_num);
    let applied = !git(["stash-applied", &stash_name])
        .status()
        .unwrap()
        .success();
    if !applied {
        print!("Stash may not be applied. Drop anyway? [y/N] ");
        io::stdout().flush().unwrap();
        if read_line() != "y" {
            return
        }
    }
    git(["stash", "drop", &stash_name]);
}

fn commit_to_branch(stash_num: u32, can_save_branch: bool) {
    let stash_name = stash_ref(stash_num);
    if !can_save_branch {
        eprintln!(
            "{TTY_BOLD_RED}\
            ERROR - Can't commit branches with unstaged files!.\
            {TTY_CLEAR}"
        );
        return;
    }
    let commit_msg_file = ".git/COMMIT_EDITMSG";

    let branch_name = "stash/__TEMP_STASH__";
    git(["checkout", "-b", branch_name]);
    git(["stash", "apply", &stash_name]);
    git(["add", "."]);
    if !git(["commit", "-n"]).status().unwrap().success() {
        git(["reset", "HEAD"]);
        git(["checkout", "."]);
        git(["clean", "-f"]);
        git(["checkout", "-"]);
        git(["branch", "-d", branch_name]);
        return;
    }

    // Change the branch name to the first line of the commit message.
    let file = File::open(commit_msg_file).unwrap();
    let reader = BufReader::new(file);
    let subject = reader.lines().find_map(|line| {
        let line = line.unwrap();
        (!line.is_empty() && !line.starts_with('#')).then(|| line)
    }).unwrap();

    let subject_terms: Vec<_> = subject.trim().split_whitespace().collect();
    let mut subject = subject_terms.join("_");
    subject.retain(|c| c == '_' || c.is_alphanumeric());
    let new_branch_name = format!("stash/{}", &subject.to_lowercase());

    git(["branch", "-m", &new_branch_name]);
    git(["checkout", "-"]);
    git(["stash", "drop", &stash_name]);
}

fn main() {
    let can_save_branch = !has_local_changes();
    if !can_save_branch {
        eprintln!(
            "{TTY_BOLD_RED}\
            WARNING - Can't backup stashes as branches with local changes.\n\
            Resolve local changes to backup stashes as branches.\
            {TTY_CLEAR}"
        );
    }
    let mut stash_num = 0;
    let mut once = true;
    loop {
        if !git_stash_show(stash_num) {
            if !once {
                println!("No stashes found.");
            }
            break;
        }
        once = true;
        print!("{TTY_BOLD_BLUE}Action on this stash [d,b,s,a,q,?]? {TTY_CLEAR}");
        io::stdout().flush().unwrap();
        let action = read_line();
        match action.as_str() {
            "d" => drop_stash(stash_num),
            "b" => commit_to_branch(stash_num, can_save_branch),
            "s" => { stash_num += 1; }
            "a" => {
                git(["stash", "apply", &stash_ref(stash_num)]);
                break;
            }
            "q" => { break; }
            "?" => {
                println!(
                    "{TTY_BOLD_RED}\
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
}
