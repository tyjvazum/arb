Moderation
==========

`arb` includes a block explorer, which you can run locally with `arb server`.

The block explorer allows viewing inscriptions. Inscriptions are user-generated
content, which may be objectionable or unlawful.

It is the responsibility of each individual who runs an ordinal block explorer
instance to understand their responsibilities with respect to unlawful content,
and decide what moderation policy is appropriate for their instance.

In order to prevent particular inscriptions from being displayed on an `arb`
instance, they can be included in a YAML config file, which is loaded with the
`--config` option.

To hide inscriptions, first create a config file, with the inscription ID you
want to hide:

```yaml
hidden:
- 0000000000000000000000000000000000000000000000000000000000000000i0
```

The suggested name for `arb` config files is `arb.yaml`, but any filename can
be used.

Then pass the file to `--config` when starting the server:

`arb --config arb.yaml server`

Note that the `--config` option comes after `arb` but before the `server`
subcommand.

`arb` must be restarted in to load changes to the config file.
