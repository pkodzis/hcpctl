//! Readiness-driven process reporting without blocking a cancellation task.

#[cfg(unix)]
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
#[cfg(unix)]
use std::fs::File;
use std::io;
#[cfg(unix)]
use std::pin::Pin;
#[cfg(unix)]
use std::task::{Context, Poll};
#[cfg(unix)]
use tokio::io::unix::AsyncFd;
#[cfg(unix)]
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

/// Duplicates a process descriptor, restoring its shared status flags on drop.
/// No blocking-pool task is created for pipe writes.
#[cfg(unix)]
pub struct ProcessReporter {
    fd: AsyncFd<File>,
    original_flags: OFlags,
}

#[cfg(unix)]
impl ProcessReporter {
    pub fn new(stderr: bool) -> io::Result<Self> {
        let fd = if stderr {
            rustix::io::dup(io::stderr())?
        } else {
            rustix::io::dup(io::stdout())?
        };
        Self::from_file(File::from(fd))
    }

    fn from_file(file: File) -> io::Result<Self> {
        let original_flags = fcntl_getfl(&file)?;
        let registered_file = file.try_clone()?;
        fcntl_setfl(&file, original_flags | OFlags::NONBLOCK)?;
        // Regular files cannot be registered with epoll. They are handled by
        // a separate variant below, never by a blocking-pool pipe writer.
        match AsyncFd::new(registered_file) {
            Ok(fd) => Ok(Self { fd, original_flags }),
            Err(error) => {
                let _ = fcntl_setfl(&file, original_flags);
                Err(error)
            }
        }
    }
}

#[cfg(unix)]
impl Drop for ProcessReporter {
    fn drop(&mut self) {
        let _ = fcntl_setfl(self.fd.get_ref(), self.original_flags);
    }
}

#[cfg(unix)]
impl AsyncWrite for ProcessReporter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut ready = std::task::ready!(self.fd.poll_write_ready(cx))?;
            match ready.try_io(|fd| Ok(rustix::io::write(fd.get_ref(), bytes)?)) {
                Ok(result) => return Poll::Ready(result),
                Err(_) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

/// Best-effort final/update diagnostics must not turn a result into a panic or
/// hang indefinitely on an unread pipe. Bulk progress uses its cancellation
/// token instead of this short final-message deadline.
pub async fn report_stderr(message: &str) {
    #[cfg(unix)]
    match ProcessReporter::new(true) {
        Ok(mut reporter) => {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                reporter.write_all(message.as_bytes()),
            )
            .await;
            return;
        }
        Err(error) if error.raw_os_error() == Some(rustix::io::Errno::PERM.raw_os_error()) => {}
        Err(_) => return,
    }
    // File redirection (and non-Unix) fallback; regular-file I/O can still be
    // held by the operating system. There are no bulk filesystem workers here.
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        tokio::io::stderr().write_all(message.as_bytes()),
    )
    .await;
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_pipe_write_is_pending_droppable_and_restores_flags() {
        let (_read, write) = rustix::pipe::pipe().unwrap();
        let file = File::from(write);
        let original = fcntl_getfl(&file).unwrap();
        let mut reporter = ProcessReporter::from_file(file.try_clone().unwrap()).unwrap();
        let bytes = [0u8; 4096];
        loop {
            match rustix::io::write(&file, &bytes) {
                Ok(_) => {}
                Err(rustix::io::Errno::AGAIN) => break,
                Err(error) => panic!("{error}"),
            }
        }
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(20),
            reporter.write_all(b"blocked")
        )
        .await
        .is_err());
        drop(reporter);
        assert_eq!(fcntl_getfl(&file).unwrap(), original);
        // No started spawn_blocking task survives the canceled write future.
    }

    #[tokio::test]
    async fn closed_pipe_returns_broken_pipe_without_panicking() {
        let (read, write) = rustix::pipe::pipe().unwrap();
        drop(read);
        let mut reporter = ProcessReporter::from_file(File::from(write)).unwrap();
        assert_eq!(
            reporter
                .write_all(b"notification")
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
