# kernelstats

This project contains a Rust application that extracts statistics about Linux
releases from a git repository, and a Jupyter notebook for setting up
interesting visualizations.

## Dependencies

 * git
 * [tokei](https://github.com/Aaronepower/tokei)
 * [Jupyter](http://jupyter.org/) with [matplotlib](https://matplotlib.org/)

## Features

 * Iterates over git tags and generate statistics per-tag (`--kernel-git`).
 * Downloads tarballs for [older
   releases](https://mirrors.kernel.org/pub/linux/kernel) based on
   [`src/kernels.yaml`](src/kernels.yaml).

## Extracting and Plotting

**Warning:** Running kernelstats against an existing git directory is currently
destructive to any changes in that directory.

Run kernelstats:

```
RUST_LOG=info cargo run -- --kernel-git path/to/linux.git
```

Start Jupyter Notebook:

```
jupyter notebook
```

## Images Generated

**Note:** Click for bigger images.

#### Percentage distributions

[![Percentages](gfx/kernelstats-percentages.png)](https://raw.githubusercontent.com/udoprog/kernelstats/master/gfx/kernelstats-percentages.png)

#### Millions of lines of code over time

[![Millions](gfx/kernelstats-millions.png)](https://raw.githubusercontent.com/udoprog/kernelstats/master/gfx/kernelstats-millions.png)
