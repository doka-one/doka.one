pub(crate) fn has_not_printable_char(tag_name: &str) -> bool {
    use unicode_segmentation::UnicodeSegmentation;
    let mut g_str = tag_name.graphemes(true);

    loop {
        let o_s = g_str.next();
        match o_s {
            None => {
                break;
            }
            Some(c) => {
                for cc in c.chars() {
                    let val = cc as u32;
                    if val == 32 || val <= 15 {
                        return true;
                    }
                }
            }
        }
    }
    false
}
