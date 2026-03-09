use {
    std::{
        env,
        io,
    },
    winres::WindowsResource,
};

#[cfg(target_os = "linux")]
fn configure_rpath() {
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}

#[cfg(not(target_os = "linux"))]
fn configure_rpath() {
    // No action needed for other operating systems
}

fn main() -> io::Result<()> {
    // Configure rpath if the target OS is Linux
    configure_rpath();

    // Check if the build is targeting Windows
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // If on Windows, compile the Windows resources
        if let Err(e) = WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile()
        {
            eprintln!("Error compiling Windows resources: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}