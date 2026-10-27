//! Embeds `assets/icon.ico` as the executable icon shown by Explorer and the taskbar.
//! The compiled resource (.res) is written by hand and handed to the MSVC linker,
//! so no `rc.exe` / Windows SDK is required.

use std::{env, fs, path::PathBuf};

const ICON: &str = "assets/icon.ico";
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

fn main() {
    println!("cargo:rerun-if-changed={ICON}");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let ico = fs::read(ICON).expect("read icon");
    let u16_at = |i: usize| u16::from_le_bytes([ico[i], ico[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(ico[i..i + 4].try_into().unwrap());

    let mut res = Vec::new();
    // A .res file starts with an empty entry.
    resource(&mut res, 0, 0, 0, &[]);

    let count = u16_at(4);
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&count.to_le_bytes());
    for i in 0..count {
        let e = 6 + 16 * i as usize;
        let (size, offset) = (u32_at(e + 8), u32_at(e + 12) as usize);
        let id = i + 1;
        resource(&mut res, RT_ICON, id, 0x1010, &ico[offset..offset + size as usize]);
        // GRPICONDIRENTRY: the ICONDIRENTRY fields with the file offset replaced by the resource id.
        group.extend_from_slice(&ico[e..e + 12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("icon.res");
    fs::write(&out, res).expect("write icon.res");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// Appends one resource with numeric type and name, language neutral.
fn resource(res: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    res.extend_from_slice(&(data.len() as u32).to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes()); // header size
    res.extend_from_slice(&[0xff, 0xff]);
    res.extend_from_slice(&kind.to_le_bytes());
    res.extend_from_slice(&[0xff, 0xff]);
    res.extend_from_slice(&name.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes()); // data version
    res.extend_from_slice(&flags.to_le_bytes());
    res.extend_from_slice(&0u16.to_le_bytes()); // language
    res.extend_from_slice(&0u32.to_le_bytes()); // version
    res.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    res.extend_from_slice(data);
    res.resize(res.len().next_multiple_of(4), 0);
}
