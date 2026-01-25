pub(super) fn metadata_to_pairs(md: &tonic::metadata::MetadataMap) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for key_and_val in md.iter() {
        if let tonic::metadata::KeyAndValueRef::Ascii(key, value) = key_and_val
            && let Ok(v) = value.to_str()
        {
            out.push((key.as_str().to_string(), v.to_string()));
        }
    }
    out
}
