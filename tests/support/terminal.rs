use portable_pty::{Child, CommandBuilder, PtySize};
use std::{
    io::{self, Read, Write},
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const ROWS: u16 = 40;
const COLUMNS: u16 = 140;
const OUTPUT_CHUNK: usize = 8192;
const OUTPUT_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) struct Terminal {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    output: Receiver<io::Result<Vec<u8>>>,
    parser: vt100::Parser,
}

impl Terminal {
    pub(super) fn start(directory: &Path, arguments: &[&str]) -> Self {
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLUMNS,
                ..Default::default()
            })
            .expect("the test can open a pseudo-terminal");
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_boom"));
        command.args(arguments);
        command.env("HOME", directory);
        command.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(command)
            .expect("boom starts in the PTY");
        let mut reader = pair
            .master
            .try_clone_reader()
            .expect("the PTY exposes output");
        let writer = pair.master.take_writer().expect("the PTY exposes input");
        let (sender, output) = mpsc::channel();
        thread::spawn(move || read_output(reader.as_mut(), &sender));
        Self {
            child,
            writer,
            output,
            parser: vt100::Parser::new(ROWS, COLUMNS, 0),
        }
    }

    pub(super) fn send(&mut self, input: &str) {
        self.writer
            .write_all(input.as_bytes())
            .expect("boom accepts terminal input");
        self.writer.flush().expect("terminal input flushes");
    }

    pub(super) fn paste(&mut self, text: &str) {
        self.send(&format!("\x1b[200~{text}\x1b[201~"));
    }

    pub(super) fn wait(&mut self, description: &str, ready: impl Fn(&str) -> bool) -> String {
        loop {
            let contents = self.parser.screen().contents();
            if ready(&contents) {
                return contents;
            }
            let bytes = self
                .output
                .recv_timeout(OUTPUT_TIMEOUT)
                .unwrap_or_else(|error| panic!("Waiting for {description}: {error}\n{contents}"));
            let bytes = bytes.expect("the PTY reader returns output");
            assert_ne!(
                bytes.len(),
                0,
                "Exited while waiting for {description}\n{contents}"
            );
            self.parser.process(&bytes);
        }
    }

    pub(super) fn click(&mut self, text: &str) {
        let (row, line) = self
            .parser
            .screen()
            .rows(0, COLUMNS)
            .enumerate()
            .find(|(_, line)| line.contains(text))
            .expect("the click target appears in the terminal");
        let prefix = line
            .find(text)
            .expect("the selected row contains the click target");
        let column = line[..prefix].chars().count() + 1;
        let row = row + 1;
        self.send(&format!("\x1b[<0;{column};{row}M\x1b[<0;{column};{row}m"));
    }

    pub(super) fn close(&mut self) {
        self.send("\x03");
        assert_eq!(
            self.child
                .wait()
                .expect("boom exits after Ctrl+C")
                .exit_code(),
            0
        );
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self
            .child
            .try_wait()
            .expect("the child status is available")
            .is_none()
        {
            self.child
                .kill()
                .expect("the test can stop its child process");
            self.child.wait().expect("the stopped child exits");
        }
    }
}

fn read_output(reader: &mut dyn Read, sender: &Sender<io::Result<Vec<u8>>>) {
    loop {
        let mut buffer = [0; OUTPUT_CHUNK];
        let output = reader
            .read(&mut buffer)
            .map(|length| buffer[..length].to_vec());
        let ended = match &output {
            Ok(bytes) => bytes.is_empty(),
            Err(_) => true,
        };
        // Closing the test session drops its receiver while the reader finishes.
        if sender.send(output).is_err() || ended {
            return;
        }
    }
}
