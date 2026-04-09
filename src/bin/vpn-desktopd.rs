use anyhow::Result;

fn main() -> Result<()> {
    vpn_desktop::daemon::run()
}
