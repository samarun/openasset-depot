using System;

namespace OpenAssetDepot.Unity
{
    [Serializable]
    internal sealed class FileStatus
    {
        public string path;
        public string local_state;
        public string pending_action;
        public int local_revision;
        public int remote_revision;
        public bool remote_deleted;
        public bool needs_sync;
        public string lock_state;
        public string lock_reason;
    }

    [Serializable]
    internal sealed class StatusData
    {
        public FileStatus[] files;
    }

    [Serializable]
    internal sealed class StatusEnvelope
    {
        public int protocol_version;
        public bool ok;
        public StatusData data;
        public string error;
    }

    [Serializable]
    internal sealed class ActionData
    {
        public string path;
        public string action;
        public string changelist_id;
    }

    [Serializable]
    internal sealed class ActionEnvelope
    {
        public int protocol_version;
        public bool ok;
        public ActionData data;
        public string error;
    }

    [Serializable]
    internal sealed class SyncEntry
    {
        public string path;
        public int revision_number;
        public bool deleted;
    }

    [Serializable]
    internal sealed class SyncData
    {
        public SyncEntry[] synced;
    }

    [Serializable]
    internal sealed class SyncEnvelope
    {
        public int protocol_version;
        public bool ok;
        public SyncData data;
        public string error;
    }

    [Serializable]
    internal sealed class SubmittedRevision
    {
        public string path;
        public int revision_number;
    }

    [Serializable]
    internal sealed class SubmitData
    {
        public string changelist_id;
        public SubmittedRevision[] revisions;
    }

    [Serializable]
    internal sealed class SubmitEnvelope
    {
        public int protocol_version;
        public bool ok;
        public SubmitData data;
        public string error;
    }

    [Serializable]
    internal sealed class ValidationList
    {
        public ValidationMessage[] warnings;
        public ValidationMessage[] errors;
    }

    [Serializable]
    internal sealed class ValidationMessage
    {
        public string path;
        public string code;
        public string message;
    }

    [Serializable]
    internal sealed class ValidationData
    {
        public ValidationList adapter;
        public ValidationList core;
    }

    [Serializable]
    internal sealed class ValidationEnvelope
    {
        public int protocol_version;
        public bool ok;
        public ValidationData data;
        public string error;
    }

    [Serializable]
    internal sealed class ShelvedFile
    {
        public string path;
        public string action;
    }

    [Serializable]
    internal sealed class ShelfData
    {
        public ShelvedFile[] files;
    }

    [Serializable]
    internal sealed class ShelfEnvelope
    {
        public int protocol_version;
        public bool ok;
        public ShelfData data;
        public string error;
    }

    [Serializable]
    internal sealed class UnshelveData
    {
        public int restored_count;
        public string[] written;
    }

    [Serializable]
    internal sealed class UnshelveEnvelope
    {
        public int protocol_version;
        public bool ok;
        public UnshelveData data;
        public string error;
    }
}
