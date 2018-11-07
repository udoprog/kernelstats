# kernelstats

This project contains a Rust application that extracts statistics about Linux releases from
a git repository, and a Jupyter notebook for setting up interesting visualizations.

## Dependencies

 * git
 * [tokei](https://github.com/Aaronepower/tokei)
 * [Jupyter](http://jupyter.org/) with [matplotlib](https://matplotlib.org/)

## Extracting and Plotting

Run kernelstats:

```
cargo run +nightly -- path/to/linux.git
```

Start Jupyter Notebook:

```
jupyter notebook
```
