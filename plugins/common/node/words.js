'use strict';

/**
 * Canonical user-facing wording for the JavaScript host panels.
 *
 * This is the Node counterpart of `openasset_depot_bridge/words.py` and must
 * stay identical to it; `plugins/common/tests/test_words.py` compares the two so
 * the Adobe and Resolve panels cannot drift from the Python hosts.
 *
 * See `docs/glossary.md` for what each term means and why.
 */

const PRODUCT_NAME = 'OpenAsset Depot';
const SHORT_NAME = 'Depot';

const ACTIONS = Object.freeze({
  refresh: 'Refresh Status',
  checkout: 'Check Out',
  add: 'Add to Depot',
  sync: 'Sync Latest',
  validate: 'Validate',
  submit: 'Submit Changes',
  // "Revert Intent" rather than "Revert Checkout": this drops the pending
  // source-control operation and releases the lock but leaves the local file
  // alone, which "Revert" on its own does not communicate.
  revert: 'Revert Intent',
  unlock: 'Release Lock',
  shelve: 'Shelve Changes',
  unshelve: 'Restore Shelf',
  history: 'View History',
});

const STATUSES = Object.freeze({
  up_to_date: 'Up to Date',
  needs_sync: 'Needs Sync',
  checked_out: 'Checked Out',
  in_use: 'In Use',
  ready_to_submit: 'Ready to Submit',
  blocked: 'Blocked',
  marked_for_delete: 'Marked for Delete',
  new_file: 'New File',
});

const PROGRESS = Object.freeze({
  refresh: 'Refreshing status',
  checkout: 'Checking out',
  add: 'Adding to depot',
  sync: 'Downloading files',
  validate: 'Validating',
  submit: 'Submitting changes',
  revert: 'Reverting intent',
  shelve: 'Shelving changes',
  unshelve: 'Restoring shelf',
  preview: 'Uploading preview',
});

const FIELDS = Object.freeze({
  description: 'Description',
  reason: 'Reason',
  server: 'Server',
  workspace: 'Workspace',
});

const MESSAGES = Object.freeze({
  no_workspace:
    `This project is not inside an ${PRODUCT_NAME} workspace. ` +
    'Create one in the desktop app, then reopen the file from there.',
  not_signed_in:
    `Sign in to ${PRODUCT_NAME} in the desktop app or with ` +
    '`oad login`, then try again.',
  save_first: 'Save the file before running this action.',
  locked_by_other:
    'Another artist has this checked out. Ask them to submit or release ' +
    'the lock, then try again.',
  nothing_pending: 'There are no pending changes to submit.',
  nothing_to_shelve: 'There are no pending changes to shelve.',
  need_description: `Enter a ${FIELDS.description.toLowerCase()} before submitting.`,
  needs_sync_first:
    `A newer revision exists on the server. Run ${ACTIONS.sync} ` +
    'before submitting.',
});

/** Builds a host-specific label, keeping the canonical verb in front. */
function qualified(action, target) {
  const verb = ACTIONS[action];
  if (!verb) throw new Error(`unknown action: ${action}`);
  const noun = (target || '').trim();
  return noun ? `${verb} ${noun}` : verb;
}

/** Builds progress text, optionally naming what is being worked on. */
function progressFor(operation, target) {
  const phrase = PROGRESS[operation] || operation.replace(/_/g, ' ');
  return target ? `${phrase} ${target}` : phrase;
}

/**
 * Maps bridge status flags onto one canonical label.
 *
 * Mirrors `status_label` in the Python module, including the precedence: a
 * validation block outranks everything, and another artist's lock outranks the
 * caller's own pending edit because it is the part they cannot resolve alone.
 */
function statusLabel({
  needsSync = false,
  lockState = null,
  pendingAction = null,
  tracked = true,
  blocked = false,
} = {}) {
  if (blocked) return STATUSES.blocked;
  if (lockState === 'other') return STATUSES.in_use;
  if (pendingAction === 'delete') return STATUSES.marked_for_delete;
  if (pendingAction === 'add' || !tracked) return STATUSES.new_file;
  if (pendingAction) return STATUSES.ready_to_submit;
  if (lockState === 'self') return STATUSES.checked_out;
  if (needsSync) return STATUSES.needs_sync;
  return STATUSES.up_to_date;
}

/**
 * Labels a raw `oad integration status` file entry.
 *
 * Mirrors `status_from_bridge`: the CLI reports locks as `mine`/`other` and
 * trackedness as a `local_state` of `untracked`, and that translation belongs in
 * one place rather than in each panel.
 */
function statusFromBridge(file, { blocked = false } = {}) {
  return statusLabel({
    needsSync: Boolean(file.needs_sync),
    lockState: file.lock_state === 'mine' ? 'self' : file.lock_state,
    pendingAction: file.pending_action,
    tracked: file.local_state !== 'untracked',
    blocked,
  });
}

module.exports = {
  PRODUCT_NAME,
  SHORT_NAME,
  ACTIONS,
  STATUSES,
  PROGRESS,
  FIELDS,
  MESSAGES,
  qualified,
  progressFor,
  statusLabel,
  statusFromBridge,
};
