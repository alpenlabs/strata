use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "refresh")]
/// Runs any background tasks manually
pub struct RefreshArgs {}

pub async fn refresh() {
    // let seed = Seed::load_or_create().unwrap();
}
