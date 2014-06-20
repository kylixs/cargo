use std::io::File;
use std::io::timer;

use support::{ResultTest,project,execs,main_file};
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::{process};

fn setup() {
}

test!(cargo_compile_with_nested_deps_shorthand {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]

            version = "0.5.0"
            path = "baz"

            [[lib]]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("bar/baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "baz"
        "#)
        .file("bar/baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_str()
            }
        "#);

    p.cargo_process("cargo-compile")
        .exec_with_output()
        .assert();

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("test passed\n"));
})

test!(no_rebuild_dependency {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}"]
        "#, bar.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
            [dependencies.bar] version = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#);
    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_stdout(format!("Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            bar.display(),
                                            p.root().display())));
    // This time we shouldn't compile bar
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Skipping fresh bar v0.5.0 (file:{})\n\
                                             Skipping fresh foo v0.5.0 (file:{})\n",
                                            bar.display(),
                                            p.root().display())));

    p.build(); // rebuild the files (rewriting them in the process)
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            bar.display(),
                                            p.root().display())));
})

test!(deep_dependencies_trigger_rebuild {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
            [dependencies.bar] version = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "bar"
            [dependencies.baz] version = "0.5.0"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;
            pub fn bar() { baz::baz() }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_stdout(format!("Compiling baz v0.5.0 (file:{})\n\
                                             Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Skipping fresh baz v0.5.0 (file:{})\n\
                                             Skipping fresh bar v0.5.0 (file:{})\n\
                                             Skipping fresh foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));

    // Make sure an update to baz triggers a rebuild of bar
    //
    // We base recompilation off mtime, so sleep for at least a second to ensure
    // that this write will change the mtime.
    timer::sleep(1000);
    File::create(&p.root().join("baz/src/baz.rs")).write_str(r#"
        pub fn baz() { println!("hello!"); }
    "#).assert();
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Compiling baz v0.5.0 (file:{})\n\
                                             Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));

    // Make sure an update to bar doesn't trigger baz
    File::create(&p.root().join("bar/src/bar.rs")).write_str(r#"
        extern crate baz;
        pub fn bar() { println!("hello!"); baz::baz(); }
    "#).assert();
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Skipping fresh baz v0.5.0 (file:{})\n\
                                             Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));
})

test!(no_rebuild_two_deps {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
            [dependencies.bar] version = "0.5.0"
            [dependencies.baz] version = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "bar"
            [dependencies.baz] version = "0.5.0"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_stdout(format!("Compiling baz v0.5.0 (file:{})\n\
                                             Compiling bar v0.5.0 (file:{})\n\
                                             Compiling foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));
    assert_that(p.process("cargo-compile"),
                execs().with_stdout(format!("Skipping fresh baz v0.5.0 (file:{})\n\
                                             Skipping fresh bar v0.5.0 (file:{})\n\
                                             Skipping fresh foo v0.5.0 (file:{})\n",
                                            baz.display(),
                                            bar.display(),
                                            p.root().display())));
})
