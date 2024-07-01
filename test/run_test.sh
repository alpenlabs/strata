pushd .. > /dev/null
# if [ "$CARGO_DEBUG" = 1 ]; then
# 	export PATH=$(realpath target/debug/):$PATH
# else
# 	export PATH=$(realpath target/release/):$PATH
# fi
cargo build
export PATH=$(realpath target/debug/):$PATH
popd > /dev/null

poetry run python entry.py $@

