namespace OpenAssetDepot.Unity
{
    /// <summary>
    /// Canonical user-facing wording, kept identical to
    /// plugins/common/openasset_depot_bridge/words.py.
    ///
    /// C# cannot import the Python or Node word list, so this is the one place
    /// the Unity integration is allowed to spell these labels. Changing a term
    /// means changing it there first; scripts/check-terminology.py fails CI if
    /// this file reintroduces a retired synonym.
    /// </summary>
    internal static class OpenAssetWords
    {
        internal const string ProductName = "OpenAsset Depot";
        internal const string ShortName = "Depot";

        internal const string Refresh = "Refresh Status";
        internal const string Checkout = "Check Out";
        internal const string Add = "Add to Depot";
        internal const string Sync = "Sync Latest";
        internal const string Validate = "Validate";
        internal const string Submit = "Submit Changes";
        internal const string Shelve = "Shelve Changes";
        internal const string Unshelve = "Restore Shelf";
        internal const string Revert = "Revert Intent";
        internal const string Unlock = "Release Lock";
        internal const string History = "View History";

        internal const string UpToDate = "Up to Date";
        internal const string NeedsSync = "Needs Sync";
        internal const string CheckedOut = "Checked Out";
        internal const string InUse = "In Use";
        internal const string ReadyToSubmit = "Ready to Submit";
        internal const string Blocked = "Blocked";
        internal const string MarkedForDelete = "Marked for Delete";
        internal const string NewFile = "New File";

        internal const string DescriptionField = "Description";
        internal const string ReasonField = "Reason";
        internal const string ServerField = "Server";
        internal const string WorkspaceField = "Workspace";
    }
}
