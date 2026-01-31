# EXAMPLES

Run 'uptime' on hosts from config file at `~/.config/bdsh/hosts`:

    bdsh -- uptime

Run on specific hosts inline:

    bdsh web1,web2,db1 -- df -h

Run on hosts tagged :web from config:

    bdsh :web -- systemctl status nginx

Run on hosts from a file:

    bdsh @./my-hosts.txt -- whoami

Run on hosts from a command:

    bdsh @'aws ec2 describe-instances --query ...' -- uname -a

Watch existing output directory:

    bdsh --watch /tmp/bdsh-output

# HOST SOURCES

If no source is specified, hosts are loaded from `~/.config/bdsh/hosts`.

- **@/path/to/file** - Read hosts from file. If executable, run it and parse stdout.
- **@'command'** - Run shell command, parse stdout as hosts.
- **host1,host2,...** - Inline comma-separated hostnames (tags not supported).

# HOSTS FILE FORMAT

The hosts file contains one host per line with optional tags:

    # Web servers
    web1.example.com :web :prod
    web2.example.com :web :prod

    # Database servers
    db1.example.com :db :prod
    db2.example.com :db :staging

    # Untagged host
    admin.example.com

Lines starting with `#`, `//`, or `;` are comments. Tags start with `:` and
are whitespace-separated after the hostname. If the hosts file is executable,
bdsh runs it and parses stdout.

# TAG FILTERING

Tags filter which hosts to target. Filters are the second positional argument,
or the first argument if it starts with ':'.

- **:tag** - Hosts with tag
- **:t1:t2** - Hosts with t1 AND t2
- **:t1,:t2** - Hosts with t1 OR t2
- **:t1:t2,:t3** - Hosts with (t1 AND t2) OR t3

Examples:

    bdsh :web -- cmd              # Hosts tagged :web
    bdsh :web:prod -- cmd         # Hosts with BOTH :web AND :prod
    bdsh :web,:db -- cmd          # Hosts with :web OR :db
    bdsh @hosts.txt :prod -- cmd  # From file, filtered by :prod

# WATCH MODE

The watch window (window 0) shows a live consensus view of all host output.
Lines that differ between hosts are highlighted. Press 'q' to quit, 'K' to
keep the output directory, or switch to a host window for full interaction.

Use `--watch` to view an existing output directory without running commands.
Use `--no-watch` to disable the watch window entirely.

# KEYBOARD SHORTCUTS

These shortcuts work in the watch window:

- **q** - Quit and clean up
- **K** - Keep output directory on exit
- **j/k** - Scroll down/up
- **g/G** - Jump to top/bottom
- **Tab** - Cycle through hosts showing per-host view
- **1-9** - Jump to specific host

# OUTPUT DIRECTORY

By default, bdsh creates a temporary directory for output. Each host gets
a subdirectory containing:

- **out.log** - Captured terminal output
- **status** - Current status (pending/running/success/failed)
- **meta.json** - Timing and exit code information
- **command** - The wrapper script that was executed

Use `-o/--output-dir` to specify a directory, and `-k/--keep` to preserve it.

# ENVIRONMENT

- **NO_COLOR** - When set (to any value), disables colored output.

# FILES

- **~/.config/bdsh/hosts** - Default hosts file. Can be plain text or executable.

# SEE ALSO

ssh(1), tmux(1)
