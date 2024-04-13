use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let context = zmq::Context::new();

    let frontend = context
        .socket(zmq::XSUB)
        .context("Failed to create subscriber socket")?;
    frontend
        .bind("tcp://*:5556")
        .context("Failed to connect publisher socket")?;

    let backend = context
        .socket(zmq::XPUB)
        .context("Failed to create publisher socket")?;
    backend
        .bind("tcp://*:8100")
        .context("Failed to bin publisher socket")?;

    zmq::proxy(&frontend, &backend).context("Failed to start proxy")?;
    Ok(())
}
