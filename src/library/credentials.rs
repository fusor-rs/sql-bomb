use super::{Error, Library};
use rusqlite::Transaction;
use std::rc::Rc;

pub(super) type CredentialStore = dyn Fn(&str) -> keyring::Result<Rc<keyring::Entry>>;

#[derive(Clone, Copy)]
pub(super) enum CredentialChange<'a> {
    Keep,
    Store(&'a str),
    Forget,
}

impl Library {
    pub(super) fn commit_credentials(
        &self,
        transaction: Transaction<'_>,
        account: &str,
        change: CredentialChange<'_>,
    ) -> Result<(), Error> {
        if matches!(change, CredentialChange::Keep) {
            transaction.commit()?;
            return Ok(());
        }
        let entry = (self.credentials)(account)?;
        let previous = read(&entry)?;
        match change {
            CredentialChange::Store(headers) => entry.set_password(headers)?,
            CredentialChange::Forget => forget(&entry)?,
            CredentialChange::Keep => unreachable!("unchanged credentials commit without a store"),
        }
        if let Err(error) = transaction.commit() {
            let restored = match previous {
                Some(headers) => entry.set_password(&headers),
                None => forget(&entry),
            };
            restored.map_err(|_| Error::CredentialRollback)?;
            return Err(error.into());
        }
        Ok(())
    }
}

pub(super) fn read(entry: &keyring::Entry) -> keyring::Result<Option<String>> {
    match entry.get_password() {
        Ok(headers) => Ok(Some(headers)),
        // Credentials can be removed independently in the OS credential store.
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
    }
}

fn forget(entry: &keyring::Entry) -> keyring::Result<()> {
    match entry.delete_credential() {
        // Deleting an already-removed credential leaves the intended state.
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error),
    }
}
