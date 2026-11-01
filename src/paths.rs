// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! XDG base directory helpers.
//!
//! Largely based on the spec: https://specifications.freedesktop.org/basedir/latest
//!
//! In the future, most of this can be replaced by standard library functions.
//! See:
//! * https://doc.rust-lang.org/std/os/unix/xdg/fn.config_home_dir.html
//! * https://doc.rust-lang.org/std/os/unix/xdg/fn.data_home_dir.html

use std::{env, path::PathBuf, sync::LazyLock};

// XDG_CONFIG_HOME paths

pub static COSMIC_CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| XDG_CONFIG_HOME.join("cosmic"));
pub static COSMIC_SETUP_DONE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| XDG_CONFIG_HOME.join("cosmic-initial-setup-done"));
pub static GNOME_SETUP_DONE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| XDG_CONFIG_HOME.join("gnome-initial-setup-done"));

static XDG_CONFIG_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    env::var_os("XDG_CONFIG_HOME")
        .map(Into::into)
        .unwrap_or_else(|| {
            // XXX: home_dir only returns None in extremely weird circumstances. May be better to just
            // unwrap.
            let home_dir = env::home_dir().unwrap_or_default();
            home_dir.join(".config")
        })
});

// XDG_DATA_DIRS paths

pub static DEFAULT_ICON_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    data_dir_path("pixmaps/faces/pop-robot.png")
        .unwrap_or_else(|| "/usr/share/pixmaps/faces/pop-robot.png".into())
});
pub static LAYOUTS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    data_dir_path("cosmic-layouts").unwrap_or_else(|| "/usr/share/cosmic-layouts/".into())
});
pub static THEMES_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    data_dir_path("cosmic-themes").unwrap_or_else(|| "/usr/share/cosmic-themes/".into())
});

static XDG_DATA_DIRS: LazyLock<Box<[PathBuf]>> = LazyLock::new(|| {
    let dirs = env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    env::split_paths(&dirs)
        .filter(|dir| dir.is_dir())
        .collect::<Vec<_>>()
        .into_boxed_slice()
});

fn data_dir_path(path: &str) -> Option<PathBuf> {
    for dir in &*XDG_DATA_DIRS {
        let path = dir.join(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
