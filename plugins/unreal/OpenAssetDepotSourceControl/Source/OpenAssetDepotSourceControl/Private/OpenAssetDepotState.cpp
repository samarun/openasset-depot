#include "OpenAssetDepotState.h"

#include "Dom/JsonObject.h"
#include "Styling/AppStyle.h"

FOpenAssetDepotState::FOpenAssetDepotState(FString InFilename)
    : Filename(MoveTemp(InFilename)), TimeStamp(FDateTime::UtcNow())
{
}

void FOpenAssetDepotState::UpdateFromJson(const TSharedPtr<FJsonObject>& Json)
{
    if (!Json)
    {
        return;
    }
    FString LocalState;
    FString PendingAction;
    FString LockState;
    bool bNeedsSync = false;
    bool bRemoteDeleted = false;
    LocalRevision = 0;
    RemoteRevision = 0;
    Json->TryGetStringField(TEXT("local_state"), LocalState);
    Json->TryGetStringField(TEXT("pending_action"), PendingAction);
    Json->TryGetStringField(TEXT("lock_state"), LockState);
    Json->TryGetNumberField(TEXT("local_revision"), LocalRevision);
    Json->TryGetNumberField(TEXT("remote_revision"), RemoteRevision);
    Json->TryGetBoolField(TEXT("needs_sync"), bNeedsSync);
    Json->TryGetBoolField(TEXT("remote_deleted"), bRemoteDeleted);
    bCurrent = !bNeedsSync;
    bSourceControlled = LocalState != TEXT("untracked") || LocalRevision > 0;
    bAdded = PendingAction == TEXT("add");
    bDeleted = PendingAction == TEXT("delete") || bRemoteDeleted;
    bCheckedOut = LockState == TEXT("mine");
    bCheckedOutOther = LockState == TEXT("other");
    bModified = LocalState == TEXT("modified") || PendingAction == TEXT("edit");
    bUnknown = false;
    OtherUser = bCheckedOutOther ? TEXT("another artist") : FString();
    TimeStamp = FDateTime::UtcNow();
}

FName FOpenAssetDepotState::GetIconName() const
{
    return bCheckedOutOther ? FName(TEXT("SourceControl.CheckedOutByOtherUser"))
        : bCheckedOut ? FName(TEXT("SourceControl.CheckedOut"))
        : bAdded ? FName(TEXT("SourceControl.OpenForAdd"))
        : !bCurrent ? FName(TEXT("SourceControl.NotAtHeadRevision"))
        : FName(TEXT("SourceControl.InDepot"));
}

#if SOURCE_CONTROL_WITH_SLATE
FSlateIcon FOpenAssetDepotState::GetIcon() const
{
    return FSlateIcon(FAppStyle::GetAppStyleSetName(), GetIconName());
}
#endif

FName FOpenAssetDepotState::GetSmallIconName() const
{
    return GetIconName();
}

FText FOpenAssetDepotState::GetDisplayName() const
{
    if (bCheckedOutOther) return NSLOCTEXT("OpenAssetDepot", "CheckedOutOther", "Checked Out Elsewhere");
    if (bCheckedOut) return NSLOCTEXT("OpenAssetDepot", "CheckedOut", "Checked Out by Me");
    if (bAdded) return NSLOCTEXT("OpenAssetDepot", "Added", "Marked for Add");
    if (bDeleted) return NSLOCTEXT("OpenAssetDepot", "Deleted", "Marked for Delete");
    if (!bCurrent) return NSLOCTEXT("OpenAssetDepot", "NeedsSync", "Needs Sync");
    return NSLOCTEXT("OpenAssetDepot", "Available", "Available to Edit");
}

FText FOpenAssetDepotState::GetDisplayTooltip() const
{
    return FText::Format(
        NSLOCTEXT("OpenAssetDepot", "StateTooltip", "{0} | Local r{1} | Depot r{2}"),
        GetDisplayName(),
        FText::AsNumber(LocalRevision),
        FText::AsNumber(RemoteRevision));
}

bool FOpenAssetDepotState::IsCheckedOutOther(FString* Who) const
{
    if (Who)
    {
        *Who = OtherUser;
    }
    return bCheckedOutOther;
}
