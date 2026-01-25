pub struct ScriptOutputs {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub files: Vec<(String, String)>,
}
