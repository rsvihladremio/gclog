// Copyright 2022 Dremio
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use clap::Parser;

// this is some black magic provided by https://docs.rs/built/latest/built/
// the build.rs file at the package root will write out the built.rs
// which is then populated with a bunch of constants, in this case we are
// most interested in finding the package version so we don't have to write it in
// two places
pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
/// Arguments for dqdrust
#[derive(Parser)]
#[clap(
    author = "Ryan Svihla",
    version = built_info::PKG_VERSION,
    about = "gclog analyzes a jdk8 gc log",
    long_about = "gclog analyzes a jdk8 gc log for a first pass diagnostic, it will not find all things, but it will help with the obvious things",
)]
#[clap(propagate_version = true)]
pub struct Args {
    #[clap()]
    /// gclog file to parse
    pub file_name: String,
}
