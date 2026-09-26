# P75 lane `cli-truth` — evidence (ledger `DBT-P75-011`…`017`)

Branch `lane/cli-truth`. Scope: `persistence/src/export.rs` and
`apps/aetherctl/src/{release,sec,service_cmds,main,i18n,cli,render}.rs`. No
dependency or lockfile change. Each fix has a test that failed on the old code;
the commit message names the test and the observed failure.

| commit | what the CLI claimed | what it does now | red-before |
|---|---|---|---|
| `df7084b` | `export verify` said `verified` for any file whose embedded key signed it | reports the signer's fingerprint; `verified` against a supplied trusted key only (`UntrustedSigner` otherwise). EXPORT_V1's digest is unchanged: the header outside the signature is recorded, not re-signed, because the format is published | see commit |
| `a9e63db` | `update verify` compared the metadata with itself: "installed" came from the metadata, "now" was its own `generated_epoch` | installed version from this binary, `now` from the clock; output adds `installedVersion`, `checkedAtEpoch`, `notChecked: ["installedChannel"]` | a 9.9.0 claim and a 1970-expired file both returned `ok:true` |
| `a74e665` | `vulndb update` deleted the new DB when the manifest write failed, leaving none | stages both files, swaps with the old pair set aside, restores it on failure | `vulndb_update_that_cannot_pin_keeps_the_old_db`: old DB gone (`None`) |
| `3d7185b` | `--format both --out x.html` wrote the HTML over the JSON and reported success | typed usage error (exit 2) before anything is written | exit 0, `ok:true` |
| `037121d` | `care start` accepted a 1-character digest prefix as consent | requires the 16 characters the prompt prints | `"0"` accepted |
| `56141ec` | local `release inspect/verify` and `update verify` failures exited 5 ("the service rejected") | exit 8 (local failure), as `docs/phase28/EXIT_CODES.md` defines | `left: (5, "RejectedByService")` |
| `b0e5b76`, `d6bcb93` | `--lang ar` was parsed and dropped | Arabic labels, outcome line, error text and usage headings from the catalog; JSON unchanged | see below |

## `--lang ar`

`Config.lang` carries the resolved language (`--lang` > `AETHERCORE_LANG` >
`en`) to the render funnel. JSON output never reads it. English text is
byte-for-byte the pre-catalog output: `field_label` and `value_text` return the
raw field and value for `Lang::En`. `render::tests::english_text_is_the_pre_catalog_rendering`
renders catalog-covered fields and values in English and compares them with
the old format. It fails if either function translates in English, as the WIP
version did (`recordCount` would have become its catalog label).

`ok.keysGenerated` said `(seed 0600)` in both catalogs. On Windows the seed
file inherits its directory's ACL (`DBT-P75-005`), so the claim was removed from
both languages. The command's JSON still reports the measured permissions.

## Exit-code change — owner's attention

`56141ec` changes observed exit codes (5 → 8) for three commands. It follows
the published registry rather than breaking it. Scripts that matched 5 on these
commands will see 8. It is a separate commit so it can be reverted alone.
