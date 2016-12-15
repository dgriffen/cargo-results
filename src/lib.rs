#[macro_use]
extern crate nom;
use std::str;

use nom::{
    line_ending,
    digit,
    space
};

named!(
    rest_of_line<&str>,
    do_parse!(
        content: map_res!(
            nom::not_line_ending,
            str::from_utf8
        ) >>
        line_ending >>
        (content)
    )
);

named!(
    compiling_opt<Option<()> >,
    opt!(
        do_parse!(
            ws!(tag!("Compiling")) >>
            rest_of_line >>
            ()
        )
    )
);

named!(
    finished<()>,
    do_parse!(
        ws!(tag!("Finished")) >>
        rest_of_line >>
        ()
    )
);

named!(
    suite_line<&str>,
    do_parse!(
        ws!(
            alt!(tag!("Running") | tag!("Doc-tests"))
        ) >>
        name: rest_of_line >>
        (name)
    )
);

named!(
    suite_count<()>,
    do_parse!(
        ws!(tag!("running")) >>
        rest_of_line >>
        ()
    )
);

named!(
    ok<&str>,
    map!(tag!("ok"),
    |_| "pass")
);

named!(
    failed<&str>,
    map!(tag!("FAILED"),
    |_| "fail")
);

named!(
    ok_or_failed<&str>,
    alt!(ok | failed)
);

#[derive(Debug, PartialEq)]
pub struct Test<'a, 'b> {
    pub name: &'a str,
    pub status: &'b str
}

named!(
    test_result<Test>,
    do_parse!(
        tag!("test") >>
        space >>
        name: map_res!(
            take_until_s!(" ..."),
            str::from_utf8
        ) >>
        tag!(" ...") >>
        status: ws!(ok_or_failed) >>
        (Test {
            name: name,
            status: status
        })
    )
);

named!(
    test_results<Vec<Test> >,
    many0!(
        test_result
    )
);

named!(
    digits<i64>,
    map_res!(
        map_res!(
            ws!(digit),
            str::from_utf8
        ),
        str::FromStr::from_str
    )
);

#[derive(Debug, PartialEq)]
pub struct SuiteResult<'a> {
  pub state: &'a str,
  pub passed: i64,
  pub failed: i64,
  pub ignored: i64,
  pub measured: i64
}

named!(
    suite_result<SuiteResult>,
    do_parse!(
        ws!(tag!("test result: ")) >>
        state: ok_or_failed >>
        char!('.') >>
        passed: digits >>
        tag!("passed;") >>
        failed: digits >>
        tag!("failed;") >>
        ignored: digits >>
        tag!("ignored;") >>
        measured: digits >>
        ws!(tag!("measured")) >>
        (SuiteResult {
          state:state,
          passed:passed,
          failed:failed,
          ignored:ignored,
          measured:measured
        })
    )
);

named!(
    fail_line<&str>,
    do_parse!(
        ws!(tag!("----")) >>
        name: map_res!(
            take_until!(" "),
            str::from_utf8
        ) >>
        ws!(tag!("stdout")) >>
        ws!(tag!("----")) >>
        (name)
    )
);

#[derive(Debug, PartialEq)]
pub struct Failure<'a, 'b> {
    pub name:&'a str,
    pub error:&'b str
}

named!(
    failure<Failure>,
    do_parse!(
        name: fail_line >>
        error: rest_of_line >>
        opt!(
            tag!("note: Run with `RUST_BACKTRACE=1` for a backtrace.")
        ) >>
        line_ending >>
        line_ending >>
        (Failure {
            name:name,
            error:error
        })
    )
);

named!(failures<Vec<Failure> >, many1!(failure));

named!(fail_opt<Option<Vec<Failure> > >,
    opt!(
        do_parse!(
            ws!(
                tag!("failures:")
            ) >>
            f: failures >>
            take_until!(
                "test result: "
            ) >>
            (f)
        )
    )
);

#[derive(Debug, PartialEq)]
pub struct Suite<'a, 'b, 'c, 'd, 'e, 'f> {
    pub name: &'a str,
    pub state: &'b str,
    pub passed: i64,
    pub failed: i64,
    pub failures: Option<Vec<Failure<'e, 'f>>>,
    pub ignored: i64,
    pub measured: i64,
    pub tests: Vec<Test<'c, 'd>>
}

named!(
    suite_parser<Suite>,
    do_parse!(
        name: suite_line >>
        suite_count >>
        tests: test_results >>
        failures: fail_opt >>
        r: suite_result >>
        (Suite {
            name:name,
            tests: tests,
            state: r.state,
            passed: r.passed,
            failed: r.failed,
            failures: failures,
            ignored: r.ignored,
            measured: r.measured
        })
    )
);

named!(
    suites_parser<Vec<Suite > >,
    many1!(suite_parser)
);

named!(
    pub cargo_test_result_parser<Vec<Suite > >,
    do_parse!(
        compiling_opt >>
        finished >>
        suites: suites_parser >>
        (suites)
    )
);


#[cfg(test)]
mod parser_tests {
  use nom::IResult;
  use std::fmt::Debug;
  use super::{
      compiling_opt,
      finished,
      suite_line,
      suite_count,
      ok_or_failed,
      Test,
      test_result,
      test_results,
      digits,
      suite_result,
      SuiteResult,
      cargo_test_result_parser,
      Suite,
      fail_line,
      failure,
      Failure,
      failures
  };

  fn assert_done<R:PartialEq + Debug>(l:IResult<&[u8], R>, r:R) -> () {
      assert_eq!(
          l,
          IResult::Done(&b""[..], r)
      );
  }

  #[test]
  fn it_should_match_a_compiler_line() {
      let output = &b"   Compiling docker-command v0.1.0 (file:///Users/joegrund/projects/docker-command-rs)
"[..];

      assert_done(
          compiling_opt(output),
          Some(())
      );
  }

  #[test]
  fn it_should_parse_finish_line() {
      let result = finished(
          &b"    Finished debug [unoptimized + debuginfo] target(s) in 0.0 secs
"[..]
      );

      assert_done(
          result,
          ()
      );
  }

  #[test]
  fn it_should_parse_suite_line() {
      let result = suite_line(
          &b"Running target/debug/deps/docker_command-be014e20fbd07382
"[..]
      );

      assert_done(
          result,
          "target/debug/deps/docker_command-be014e20fbd07382"
      );
  }

  #[test]
  fn it_should_parse_suite_count() {
      let result = suite_count(
          &b"running 0 tests
"[..]
      );

      assert_done(result, ());
  }

  #[test]
  fn it_should_match_ok() {
      assert_done(
        ok_or_failed(&b"ok"[..]),
        "pass"
      );
  }

  #[test]
  fn it_should_match_failed() {
      assert_done(
        ok_or_failed(&b"FAILED"[..]),
        "fail"
      );
  }

  #[test]
  fn it_should_parse_test_result() {
      let result = test_result(
          &b"test it_runs_a_command ... ok"[..]
      );

      assert_done(
          result,
          Test {
              name: "it_runs_a_command",
              status: "pass"
          }
      );
  }

  #[test]
  fn it_should_parse_test_results() {
      let result = test_results(
        &b"test tests::it_should_parse_first_line ... ok
test tests::it_should_parse_a_status_line ... ok
test tests::it_should_parse_test_output ... ok
test tests::it_should_parse_suite_line ... FAILED
"[..]
);

      assert_done(
          result,

              vec![
                Test {
                    name: "tests::it_should_parse_first_line",
                    status: "pass",
                },
                Test {
                    name: "tests::it_should_parse_a_status_line",
                    status: "pass",
                },
                Test {
                    name: "tests::it_should_parse_test_output",
                    status: "pass"
                },
                Test {
                    name: "tests::it_should_parse_suite_line",
                    status: "fail"
                }
              ]
      );
  }

    #[test]
    fn it_should_capture_digits() {
        assert_done(
            digits(b"10"),
            10
        );
    }

    #[test]
    fn it_should_parse_a_suite_result() {
      let result = suite_result(&b"test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured"[..]);

      assert_done(
        result,
        SuiteResult {
            state: "fail",
            passed: 3,
            failed: 1,
            ignored: 0,
            measured: 0,
        }
      );
    }

    #[test]
    fn it_should_parse_successful_test_output() {
        let output = &b"    Finished debug [unoptimized + debuginfo] target(s) in 0.0 secs
       Running target/debug/cargo_test_junit-83252957c74e106d

running 2 tests
test tests::it_should_match_failed ... ok
test tests::it_should_parse_first_line ... ok


  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured
  "[..];

      let result = cargo_test_result_parser(output);

      assert_done(
        result,
        vec![Suite {
            name: "target/debug/cargo_test_junit-83252957c74e106d",
            state: "pass",
            tests: vec![
                Test {
                    name: "tests::it_should_match_failed",
                    status: "pass"
                },
                Test {
                    name: "tests::it_should_parse_first_line",
                    status: "pass"
                }
            ],
            failures: None,
            passed: 2,
            failed: 0,
            ignored: 0,
            measured: 0,
        }]
      );
    }

    #[test]
    fn test_fail_line() {
        let output = b"---- fail stdout ----";

        assert_done(
            fail_line(output),
            "fail"
        );
    }

    #[test]
    fn test_failure() {
        let output = b"---- fail stdout ----
  thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

";
        assert_done(
            failure(output),
            Failure {
                name: "fail",
                error: "thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16"
            }
        );
    }

    #[test]
    fn test_failures() {
        let output = b"---- fail stdout ----
          thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

        ---- fail2 stdout ----
          thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22


";

        assert_done(
            failures(output),
            vec![
                Failure {
                    name: "fail",
                    error: "thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16"
                },
                Failure {
                    name: "fail2",
                    error: "thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22"
                }
            ]
        );
    }

    #[test]
    fn test_fail_run() {
        let output = b"  Compiling blah v0.1.0 (file:blah)
        Finished debug [unoptimized + debuginfo] target(s) in 0.32 secs
        Running target/debug/deps/docker_command-be014e20fbd07382

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

        Running target/debug/integration_test-d4fc68dd5824cbb9

running 3 tests
test fail ... FAILED
test fail2 ... FAILED
test it_runs_a_command ... ok

failures:

---- fail stdout ----
thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

---- fail2 stdout ----
thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22


failures:
        fail
        fail2

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured

error: test failed";

        let x = match cargo_test_result_parser(output) {
            IResult::Done(_, x) => x,
            _ => panic!("BOOM!")
        };


        assert_eq!(
            x,
            vec![
                Suite {
                    name: "target/debug/deps/docker_command-be014e20fbd07382",
                    state: "pass",
                    passed: 0,
                    failed: 0,
                    failures: None,
                    ignored: 0,
                    measured: 0,
                    tests: vec![]
                },
                Suite {
                    name: "target/debug/integration_test-d4fc68dd5824cbb9",
                    state: "fail",
                    passed: 1,
                    failed: 2,
                    failures: Some(
                        vec![
                            Failure {
                                name: "fail",
                                error: "thread \'fail\' panicked at \'assertion failed: `(left == right)` (left: `1`, right: `2`)\', tests/integration_test.rs:16"
                            },
                            Failure {
                                name: "fail2",
                                error: "thread \'fail2\' panicked at \'assertion failed: `(left == right)` (left: `3`, right: `2`)\', tests/integration_test.rs:22"
                            }
                        ]
                    ),
                    ignored: 0,
                    measured: 0,
                    tests: vec![
                        Test {
                            name: "fail",
                            status: "fail"
                        },
                        Test {
                            name: "fail2",
                            status: "fail"
                        },
                        Test {
                            name: "it_runs_a_command",
                            status: "pass"
                        }
                    ]
                }
            ]
        );
    }
}
