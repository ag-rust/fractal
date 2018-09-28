use i18n::i18n;

use appop::AppOp;

use backend::BKCommand;


impl AppOp {
    pub fn initial_sync(&self, show: bool) {
        if show {
            self.inapp_notify(&i18n("Syncing, this could take a while"));
            self.stickers_load();
        } else {
            self.hide_inapp_notify();
        }
    }

    pub fn sync(&mut self, initial: bool) {
        if !self.syncing && self.logged_in {
            self.syncing = true;
            self.backend.send(BKCommand::Sync(self.since.clone(), initial)).unwrap();
        }
    }

    pub fn synced(&mut self, since: Option<String>) {
        self.syncing = false;
        self.since = since;
        self.sync(false);
        self.initial_sync(false);
    }

    pub fn sync_error(&mut self) {
        self.syncing = false;
        self.sync(false);
    }
}
