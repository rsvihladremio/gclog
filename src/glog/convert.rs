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

pub fn convert_bytes_to_mb(b: usize) -> f32 {
    b as f32 / (1024.0 * 1024.0)
}
pub fn convert_bytes_to_gb(b: usize) -> f32 {
    b as f32 / (1024.0 * 1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use crate::tests::assert_approx_equal;

    use super::{convert_bytes_to_gb, convert_bytes_to_mb};

    #[test]
    fn test_convert_bytes_to_mb() {
        let res = convert_bytes_to_mb(104857);
        assert_approx_equal(0.1, res, 0.01);
    }

    #[test]
    fn test_convert_bytes_to_mb_0() {
        let res = convert_bytes_to_mb(0);
        assert_approx_equal(0.0, res, 0.01);
    }

    #[test]
    fn test_convert_bytes_to_gb() {
        let res = convert_bytes_to_gb(1073741824);
        assert_approx_equal(1.0, res, 0.01);
    }

    #[test]
    fn test_convert_bytes_to_gb_0() {
        let res = convert_bytes_to_gb(0);
        assert_approx_equal(0.0, res, 0.01);
    }
}
