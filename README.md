# Quiren (Quick Editor Renamer)

Edits the filenames of the current directory on the editor of your choice.

## Installation

From the git repository:

```shell
$ git clone https://github.com/Sinono3/quiren.git
$ cargo install --path quiren
```

## Usage

```
Usage: quiren [options] [dir]

Options:
    -h, --help      Prints help information
    -r, --retry     Re-enters the editor after an error
```

Examples:

```shell
# On the current directory
$ quiren
# On another directory
$ quiren books
$ quiren /home/dude/abc/
```

## Origin

I was looking for a tool that would let me edit filenames for the current directory in Vim. I found `massren` on the AUR, and I didn't like it at all. It was so bloated to fulfill such a simple task. Why does it automatically create a config file. Why does it a create an SQLite database (For undoing renames, but still, it should have been on the cache directory). Why do I have to scroll through a huge wall of warning texts to actually edit the filenames. Why does it take so long to actually save the changes.

Well, enough rant about that tool. If I looked a little bit more into it I probably would have found something that suited my needs, but I thought trying to make it myself would be a fun project, so here we are.
