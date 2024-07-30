# DOES NOT WORK, YET!

# bdsh

Will be, basically, [dsh](https://www.netfort.gr.jp/~dancer/software/dsh.html.en) except with useful output, and the ability to go interactive if needed.

## basic plan

1. Start a tmux session in control mode ( -C new-session -s $name )
2. for each host: start a tmux window named for the host and run ssh command in it
3. from control mode session capture all inputs
4. generate "consensus view" of output from all sessions, highlighting the ones that vary and how (diff)
5. attach to the tmux session and show the consensus view

## okay, some more details!

1. Create a temp directory to accumulate the output of all the commands and maintain some state
1. start tmux against a socket in that tmp (to avoid polluting default tmux server)
2. Run commands a la `ssh freki $command | tee $bdsh_tmp/$host/out.log` so that we capture output and still get the nice tmux experience if input is needed
3. The UI in the initial tmux fires up bdsh in "watch this directory" mode, which only needs to work off of files on disk, and not be aware of tmux :-)

Given this approach, do we even need to look at notifications in control mode? We can (maybe) just submit commands and rely on naming conventions to find things, no need to track which window is which, and so on. Caveat, need to force no duplicate host names. Are all host names valid tmux window names? I *think* so! We might want to look at notifs for error handling and timing purposes, at least (ie, how long do the commands take, to make accurate spinners).

bdsh defaults to running in "server mode", must do magical (hidden) incantation to start it in client mode against the output directory.

By default, output directory is deleted on exit, but can be preserved with a flag. It will also look for a `.keep` which the client can drop in it, so you can decide to keep it interactively from within the client. In fact, the flag just causes a `.keep` to be created!

## useful stuff

### if rust (which is overkill, but fun)

* [mitsuhiko/similar](https://github.com/mitsuhiko/similar)

## alternatives

* [dsh](https://www.netfort.gr.jp/~dancer/software/dsh.html.en)
* [pssh](https://code.google.com/archive/p/parallel-ssh/)
* [sshpt](https://code.google.com/archive/p/sshpt/)
* [clusterssh](https://github.com/duncs/clusterssh)
* [pussh](https://github.com/bearstech/pussh)
* [pdsh](https://github.com/chaos/pdsh)
