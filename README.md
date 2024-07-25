# bdsh

Basically, [dsh](https://www.netfort.gr.jp/~dancer/software/dsh.html.en) except with useful output, and the ability to go interactive if needed. 

## basic plan

1. Start a tmux session in control mode ( -C new-session -s $name )
2. for each host: start a tmux window named for the host and run ssh command in it
3. from control mode session capture all inputs
4. generate "consensus view" of output from all sessions, highlighting the ones that vary and how (diff)
5. attach to the tmux session and show the consensus view

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


