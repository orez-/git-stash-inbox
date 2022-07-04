# git-stash-inbox
Interactive tool to clean up git stashes

```diff
diff --git a/src/main.rs b/src/main.rs
index c5ae5d9..5231f3d 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -50,7 +50,7 @@ fn git_stash_show(stash_num: u32) -> io::Result<bool> {
 }

 fn read_line() -> io::Result<String> {
-    io::stdin().lock().lines().next().unwrap()
+    io::stdin().lock().lines().next().ok_or_else(|| error("out"))?
 }

 fn drop_stash(stash_num: u32) -> io::Result<()> {
Action on this stash [d,b,s,a,q,?]?
```

```
d - drop this stash
b - commit this stash to a separate branch and delete it
s - take no action on this stash
a - apply; apply the stash and take no further action
q - quit; take no further action on remaining stashes
? - print help
```
