## Flashing and running a program

```
cargo run --bin test_logging
```
replace `test_logging` with the filename you want to run.

## Logging

logging is done through the log.sh script. Usage example:
```
./log.sh target/thumbv8m.main-none-eabihf/debug/test_logging
```
replace the `test_logging` at the end with the filename you're running.

**important notes for logging**

Make sure you set up the defmt usb task. Examples are shown in `test_logging.rs` and `scan_i2c.rs`.

If nothing is printing or there's a long delay between each print, check that you're printing enough to fill the print buffer. An easy way to fix this is to have a loop that prints filler words, as seen in the previously mentioned examples.

## Examples

Reference [embassy.dev](https://embassy.dev/) for documentation and examples. The most relevant example should be in the embassy github under `examples/rp235x`.
