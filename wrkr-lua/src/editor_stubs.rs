#[derive(Debug, Clone, Copy)]
pub struct StubFile {
    pub path: &'static str,
    pub contents: &'static str,
}

pub fn luals_stub_files() -> &'static [StubFile] {
    &[
        StubFile {
            path: "wrkr/_types.lua",
            contents: include_str!("../lua-stubs/wrkr/_types.lua"),
        },
        StubFile {
            path: "wrkr/check.lua",
            contents: include_str!("../lua-stubs/wrkr/check.lua"),
        },
        StubFile {
            path: "wrkr/debug.lua",
            contents: include_str!("../lua-stubs/wrkr/debug.lua"),
        },
        StubFile {
            path: "wrkr/env.lua",
            contents: include_str!("../lua-stubs/wrkr/env.lua"),
        },
        StubFile {
            path: "wrkr/fs.lua",
            contents: include_str!("../lua-stubs/wrkr/fs.lua"),
        },
        StubFile {
            path: "wrkr/group.lua",
            contents: include_str!("../lua-stubs/wrkr/group.lua"),
        },
        StubFile {
            path: "wrkr/http.lua",
            contents: include_str!("../lua-stubs/wrkr/http.lua"),
        },
        StubFile {
            path: "wrkr/init.lua",
            contents: include_str!("../lua-stubs/wrkr/init.lua"),
        },
        StubFile {
            path: "wrkr/json.lua",
            contents: include_str!("../lua-stubs/wrkr/json.lua"),
        },
        StubFile {
            path: "wrkr/metrics.lua",
            contents: include_str!("../lua-stubs/wrkr/metrics.lua"),
        },
        StubFile {
            path: "wrkr/shared.lua",
            contents: include_str!("../lua-stubs/wrkr/shared.lua"),
        },
        StubFile {
            path: "wrkr/vu.lua",
            contents: include_str!("../lua-stubs/wrkr/vu.lua"),
        },
    ]
}
