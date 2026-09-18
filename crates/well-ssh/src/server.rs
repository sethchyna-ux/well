use russh::Error;

pub struct SSHServer {}

pub async fn run_server(_port: u16) -> Result<(), Error> {
    Ok(())
}
