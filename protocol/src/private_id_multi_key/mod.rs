//  Copyright (c) Facebook, Inc. and its affiliates.
//  SPDX-License-Identifier: Apache-2.0

extern crate csv;

use common::{files, timer};
use crypto::prelude::*;

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ProtocolError {
    ErrorDeserialization(String),
    ErrorSerialization(String),
    ErrorEncryption(String),
    ErrorCalcSetDiff(String),
    ErrorReencryption(String),
    ErrorIO(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol error {}", self)
    }
}

impl Error for ProtocolError {}

fn load_data(plaintext: Arc<RwLock<Vec<Vec<String>>>>, path: &str, input_with_headers: bool) {
    let t = timer::Timer::new_silent("load data");

    let mut lines = files::read_csv_as_strings(path, true);
    let text_len = lines.len();

    if let Ok(mut data) = plaintext.write() {
        data.clear();
        let mut line_it = lines.drain(..);
        // Strip the header for now
        if input_with_headers && line_it.next().is_some() {}

        let mut t = HashSet::<Vec<String>>::new();
        // Filter out zero length strings - these will come from ragged
        // arrays since they are padded out to the longest array
        // Also deduplicate all input
        for line in line_it {
            let v = line
                .iter()
                .map(String::from)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if !t.contains(&v) {
                data.push(v.clone());
                t.insert(v);
            }
        }
        info!("Read {} lines from {}", text_len, path,);
    }

    t.qps("text read", text_len);
}

fn writer_helper(data: &[Vec<String>], id_map: &[(String, usize, bool)], path: Option<String>) {
    let mut device = match path {
        Some(path) => {
            let wr = csv::WriterBuilder::new()
                .flexible(true)
                .buffer_capacity(1024)
                .from_path(path)
                .unwrap();
            Some(wr)
        }
        None => None,
    };

    for (key, idx, flag) in id_map.iter() {
        let mut v = vec![(*key).clone()];

        match flag {
            true => v.extend(data[*idx].clone()),
            false => v.push("NA".to_string()),
        }

        match device {
            Some(ref mut wr) => {
                wr.write_record(v.as_slice()).unwrap();
            }
            None => {
                println!("{}", v.join(","));
            }
        }
    }
}

fn compute_prefix_sum(input: &[usize]) -> Vec<usize> {
    let prefix_sum = input
        .iter()
        .scan(0, |sum, i| {
            *sum += i;
            Some(*sum)
        })
        .collect::<Vec<_>>();

    // offset is now a combined exclusive and inclusive prefix sum
    // that will help us convert to a flattened vector and back to a
    // vector of vectors
    let mut output = Vec::<usize>::with_capacity(prefix_sum.len() + 1);
    output.push(0);
    output.extend(prefix_sum);
    output
}

fn serialize_helper<T>(data: Vec<Vec<T>>) -> (Vec<T>, TPayload, TPayload) {
    let offset = {
        let lengths = data.iter().map(|v| v.len()).collect::<Vec<usize>>();
        compute_prefix_sum(&lengths)
            .iter()
            .map(|&o| ByteBuffer {
                buffer: (o as u64).to_le_bytes().to_vec(),
            })
            .collect::<Vec<_>>()
    };

    let d_flat = data.into_iter().flatten().collect::<Vec<_>>();

    let metadata = vec![
        ByteBuffer {
            buffer: (d_flat.len() as u64).to_le_bytes().to_vec(),
        },
        ByteBuffer {
            buffer: (offset.len() as u64).to_le_bytes().to_vec(),
        },
    ];

    (d_flat, offset, metadata)
}

pub mod company;
pub mod partner;
pub mod traits;
