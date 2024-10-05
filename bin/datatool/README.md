# Strata Datatool

This is a tool for doing basic operations with Strata keys and data.

## Usage

The basic flow to generate a params file with it looks like this:

```
# Generate keys for the different parties each on different machines.
strata-datatool genseed sequencer.bin
strata-datatool genseed operator1.bin
strata-datatool genseed operator2.bin

# Generate the pubkeys, also on their original machines.
strata-datatool genseqpubkey -f sequencer.bin
strata-datatool genopxpub -f operator1.bin
strata-datatool genopxpub -f operator2.bin

# Take the generated pubkeys and generate the params file with it.
strata-datatool genparams \
    -n 'hello-world-network' \
    -s XGUgTAJNpexzrjgnbMvGtDBCZEwxd6KQE4PNDWE6YLZYBTGoS \
    -b tpubDASVk1m5cxpmUbwVEZEQb8maDVx9kDxBhSLCqsKHJJmZ8htSegpHx7G3RFudZCdDLtNKTosQiBLbbFsVA45MemurWenzn16Y1ft7NkQekcD \
    -b tpubDBX9KQsqK2LMCszkDHvANftHzhJdhipe9bi9MNUD3S2bsY1ikWEZxE53VBgYN8WoNXk9g9eRzhx6UfJcQr3XqkA27aSxXvKu5TYFZJEAjCd \
    -o params.json
```

## Envvars

Alternatively, instead of passing `-f`, you can pass `-E` and define either
`STRATA_SEQ_KEY` or `STRATA_OP_KEY` to pass the seed keys to the program.

## Correst Setup for VerifyingKey
To ensure the RollupParams contain the correct verifying key, you must run the binary in release mode. This is necessary because the SP1 build process requires release mode for proper functioning.
Before proceeding, make sure that you have SP1 correctly set up by following the installation instructions provided [here](https://docs.succinct.xyz/getting-started/install.html). Additionally, refer to the [common errors section](https://docs.succinct.xyz/developers/common-issues.html#c-binding-errors) here to troubleshoot any issues, especially C binding errors.