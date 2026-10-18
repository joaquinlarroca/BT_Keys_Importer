use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;

static NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<([^>]+)>").unwrap());

fn run_chntpw(path: &Path, commands: &[&str]) -> io::Result<String> {
    let mut child: std::process::Child = Command::new("chntpw")
        .arg("-e")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin: &mut std::process::ChildStdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| io::Error::other("failed to open chntpw stdin"))?;

    for cmd in commands {
        writeln!(stdin, "{cmd}")?;
    }

    writeln!(stdin, "q")?;

    let output: std::process::Output = child.wait_with_output()?;

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn get_names(text: &str) -> Vec<&str> {
    NAME_RE
        .captures_iter(text)
        .map(|x| x.get(1).unwrap().as_str())
        .collect()
}

fn get_key(text: &str) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::with_capacity(16);

    for line in text.lines() {
        let line: &str = line.trim_start();
        let Some(rest) = line.strip_prefix("..:").or_else(|| line.strip_prefix(':')) else {
            continue;
        };

        for token in rest.split_whitespace() {
            if token.len() == 2
                && let Ok(byte) = u8::from_str_radix(token, 16)
            {
                bytes.push(byte);
                if bytes.len() == 16 {
                    return Some(bytes);
                }
            }
        }
    }

    None
}

fn to_linux_link_key(windows_bytes: &[u8]) -> String {
    windows_bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join("")
}

fn format_mac(raw: &str) -> String {
    raw.to_uppercase()
        .as_bytes()
        .chunks(2)
        .map(|x| std::str::from_utf8(x).unwrap())
        .collect::<Vec<_>>()
        .join(":")
}

fn prompt_line(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;

    let mut input: String = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

fn prompt_index(prompt: &str, items: &[&str]) -> usize {
    if items.is_empty() {
        eprintln!("No entries available to choose from");
        std::process::exit(1);
    }

    match prompt_line(prompt)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|i| *i < items.len())
    {
        Some(i) => i,
        None => {
            eprintln!("Expected value between 0 and {}", items.len() - 1);
            std::process::exit(1);
        }
    }
}

fn print_indexed(items: &[&str]) {
    for (i, item) in items.iter().enumerate() {
        println!("{i}: {item}");
    }
}

fn stop_bluetooth() -> io::Result<()> {
    let status: std::process::ExitStatus = Command::new("systemctl")
        .args(["stop", "bluetooth"])
        .status()?;

    if !status.success() {
        return Err(io::Error::other("Failed to stop bluetooth"));
    }

    Ok(())
}

fn start_bluetooth() -> io::Result<()> {
    let status: std::process::ExitStatus = Command::new("systemctl")
        .args(["start", "bluetooth"])
        .status()?;

    if !status.success() {
        return Err(io::Error::other("Failed to start bluetooth"));
    }

    Ok(())
}

fn replace_section(contents: &str, header: &str, section: &str) -> String {
    let Some(start) = contents.find(header) else {
        return contents.to_string();
    };

    let after: usize = start + header.len();
    let end: usize = contents[after..]
        .find("\n[")
        .map(|i| after + i + 1)
        .unwrap_or(contents.len());

    let mut result: String = String::with_capacity(contents.len() + section.len());
    result.push_str(&contents[..start]);
    result.push_str(section);
    result.push_str(&contents[end..]);
    result
}

fn ensure_trailing_newline(contents: &mut String) {
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
}

fn write_link_key(adapter_mac_raw: &str, device_mac_raw: &str, key_bytes: &[u8]) -> io::Result<()> {
    let adapter_mac: String = format_mac(adapter_mac_raw);
    let device_mac: String = format_mac(device_mac_raw);
    let link_key: String = to_linux_link_key(key_bytes);

    let dir: PathBuf = ["/var/lib/bluetooth", &adapter_mac, &device_mac]
        .iter()
        .collect();

    fs::create_dir_all(&dir)?;

    let info_path: PathBuf = dir.join("info");

    let mut contents: String = if info_path.exists() {
        fs::read_to_string(&info_path)?
    } else {
        String::from("[General]\nName=Unknown\n\n")
    };

    let section: String = format!("[LinkKey]\nKey={link_key}\nType=4\nPINLength=0\n");

    if contents.contains("[LinkKey]") {
        ensure_trailing_newline(&mut contents);
        contents = replace_section(&contents, "[LinkKey]", &section);
    } else {
        ensure_trailing_newline(&mut contents);
        if !contents.is_empty() && !contents.ends_with("\n\n") {
            contents.push('\n');
        }
        contents.push_str(&section);
    }

    fs::write(&info_path, contents)?;
    println!("wrote {}", info_path.display());
    Ok(())
}

fn main() -> io::Result<()> {
    let win_dir: String = prompt_line("Enter mounting point of windows dir (e.g. /mnt/windows): ")?;

    let config_dir: PathBuf = [win_dir.as_str(), "Windows", "System32", "config"]
        .iter()
        .collect();

    if !config_dir.is_dir() {
        eprintln!("No such directory: {}", config_dir.display());
        std::process::exit(1);
    }

    let system_reg: PathBuf = config_dir.join("SYSTEM");

    if !system_reg.is_file() {
        eprintln!("No such file: {}", system_reg.display());
        std::process::exit(1);
    }

    let out: String = run_chntpw(
        &system_reg,
        &[
            "cd \\ControlSet001\\Services\\BTHPORT\\Parameters\\Keys",
            "ls",
        ],
    )?;

    let adapters_macs: Vec<&str> = get_names(&out)
        .into_iter()
        .filter(|x| x.len() == 12)
        .collect();

    print_indexed(&adapters_macs);

    let adapter_mac_raw: usize =
        prompt_index("Enter the bluetooth mac(first is 0): ", &adapters_macs);

    let adapter_mac_raw: &str = adapters_macs[adapter_mac_raw];

    let dir: String = format!(
        "cd \\ControlSet001\\Services\\BTHPORT\\Parameters\\Keys\\{}",
        adapter_mac_raw
    );

    let out: String = run_chntpw(&system_reg, &[&dir, "ls"])?;

    let devices_macs: Vec<&str> = get_names(&out)
        .into_iter()
        .filter(|x: &&str| x.len() == 12)
        .collect();

    print_indexed(&devices_macs);

    let device_mac_raw: usize = prompt_index(
        "Enter the bluetooth device mac(first is 0): ",
        &devices_macs,
    );

    let device_mac_raw: &str = devices_macs[device_mac_raw];

    let out: String = run_chntpw(
        &system_reg,
        &[&dir, &format!("ed {}", device_mac_raw), "\n", "d", "q"],
    )?;

    let Some(link_key) = get_key(&out) else {
        eprintln!("link key not found");
        std::process::exit(1);
    };

    stop_bluetooth()?;

    let result: Result<(), io::Error> = write_link_key(adapter_mac_raw, device_mac_raw, &link_key);

    start_bluetooth()?;

    result
}
