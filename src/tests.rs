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

pub fn assert_approx_equal(a: f32, b: f32, tolerance: f32) {
    let ret = approx_equal(a, b, tolerance);
    if !ret {
        panic!("expected {a} but was {b}")
    }
}

pub fn approx_equal_f64(a: f64, b: f64, tolerance: f64) -> bool {
    let diff = a - b;
    diff.abs() <= tolerance
}
pub fn approx_equal(a: f32, b: f32, tolerance: f32) -> bool {
    let diff = a - b;
    diff.abs() <= tolerance
}

#[cfg(test)]
mod tesxts {
    use super::approx_equal;

    #[test]
    fn test_near_number() {
        assert!(approx_equal(0.991, 0.990, 0.1));
        assert!(approx_equal(0.991, 0.990, 0.01));
        //should fail
        assert!(!approx_equal(0.991, 0.990, 0.0001));
    }

    #[test]
    fn test_near_one_another() {
        assert!(approx_equal(0.99, 1.0, 0.1));
        assert!(approx_equal(0.99, 1.0, 0.01));
        //should fail
        assert!(!approx_equal(0.99, 1.0, 0.001));
    }
}
