// i18n.rs
//
// Copyright 2020 Christopher Davis <christopherdavis@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use gettextrs::{gettext, ngettext, npgettext, pgettext};
use lazy_regex::{Captures, Regex};

#[allow(dead_code)]
fn freplace(input: String, args: &[&str]) -> String {
    let mut parts = input.split("{}");
    let mut output = parts.next().unwrap_or_default().to_string();
    for (p, a) in parts.zip(args.iter()) {
        output += &((*a).to_string() + p);
    }
    output
}

#[allow(dead_code)]
fn kreplace(input: String, kwargs: &[(&str, &str)]) -> String {
    let mut s = input;
    for (k, v) in kwargs {
        if let Ok(re) = Regex::new(&format!("\\{{{k}\\}}")) {
            s = re
                .replace_all(&s, |_: &Captures<'_>| (*v).to_string())
                .to_string();
        }
    }

    s
}

// Simple translations functions

#[allow(dead_code)]
pub fn i18n<S: Into<String>>(format: S) -> String {
    gettext(format)
}

#[allow(dead_code)]
pub fn i18n_f<S: Into<String>>(format: S, args: &[&str]) -> String {
    let s = gettext(format);
    freplace(s, args)
}

#[allow(dead_code)]
pub fn i18n_k<S: Into<String>>(format: S, kwargs: &[(&str, &str)]) -> String {
    let s = gettext(format);
    kreplace(s, kwargs)
}

// Singular and plural translations functions

#[allow(dead_code)]
pub fn ni18n<S1: Into<String>, S2: Into<String>>(single: S1, multiple: S2, number: u32) -> String {
    ngettext(single, multiple, number)
}

#[allow(dead_code)]
pub fn ni18n_f<S1: Into<String>, S2: Into<String>>(
    single: S1,
    multiple: S2,
    number: u32,
    args: &[&str],
) -> String {
    let s = ngettext(single, multiple, number);
    freplace(s, args)
}

#[allow(dead_code)]
pub fn ni18n_k<S1: Into<String>, S2: Into<String>>(
    single: S1,
    multiple: S2,
    number: u32,
    kwargs: &[(&str, &str)],
) -> String {
    let s = ngettext(single, multiple, number);
    kreplace(s, kwargs)
}

// Translations with context functions

#[allow(dead_code)]
pub fn pi18n<S1: Into<String>, S2: Into<String>>(ctx: S1, format: S2) -> String {
    pgettext(ctx, format)
}

#[allow(dead_code)]
pub fn pi18n_f<S1: Into<String>, S2: Into<String>>(ctx: S1, format: S2, args: &[&str]) -> String {
    let s = pgettext(ctx, format);
    freplace(s, args)
}

#[allow(dead_code)]
pub fn pi18n_k<S1: Into<String>, S2: Into<String>>(
    ctx: S1,
    format: S2,
    kwargs: &[(&str, &str)],
) -> String {
    let s = pgettext(ctx, format);
    kreplace(s, kwargs)
}

// Singular and plural with context

#[allow(dead_code)]
pub fn pni18n<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
    ctx: S1,
    single: S2,
    multiple: S3,
    number: u32,
) -> String {
    npgettext(ctx, single, multiple, number)
}

#[allow(dead_code)]
pub fn pni18n_f<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
    ctx: S1,
    single: S2,
    multiple: S3,
    number: u32,
    args: &[&str],
) -> String {
    let s = npgettext(ctx, single, multiple, number);
    freplace(s, args)
}

#[allow(dead_code)]
pub fn pni18n_k<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
    ctx: S1,
    single: S2,
    multiple: S3,
    number: u32,
    kwargs: &[(&str, &str)],
) -> String {
    let s = npgettext(ctx, single, multiple, number);
    kreplace(s, kwargs)
}
