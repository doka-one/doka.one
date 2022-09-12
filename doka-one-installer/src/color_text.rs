use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

pub fn color_println(text : &str, color : Color ) -> anyhow::Result<()> {

    use std::io::Write;

    let bufwtr = BufferWriter::stderr(ColorChoice::Always);
    let mut buffer = bufwtr.buffer();
    buffer.set_color(ColorSpec::new().set_fg(Some(color)))?;

    writeln!(&mut buffer, "{}", text)?;
    bufwtr.print(&buffer)?;

    buffer.clear();
    buffer.set_color(ColorSpec::new().set_fg(None))?;

    bufwtr.print(&buffer)?;

    Ok(())
}


pub(crate) fn step_println(text : &str)  -> anyhow::Result<()> {
    color_println(text, Color::Yellow)
}

pub(crate) fn end_println(text : &str)  -> anyhow::Result<()> {
    color_println(text, Color::Green)
}
