#pragma once

#include "ISourceControlState.h"

class FOpenAssetDepotState final : public ISourceControlState
{
public:
    explicit FOpenAssetDepotState(FString InFilename);

    void UpdateFromJson(const TSharedPtr<FJsonObject>& Json);

    virtual int32 GetHistorySize() const override { return 0; }
    virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> GetHistoryItem(int32) const override { return nullptr; }
    virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FindHistoryRevision(int32) const override { return nullptr; }
    virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FindHistoryRevision(const FString&) const override { return nullptr; }
    virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> GetCurrentRevision() const override { return nullptr; }
#if SOURCE_CONTROL_WITH_SLATE
    virtual FSlateIcon GetIcon() const override;
#endif
    virtual FName GetIconName() const override;
    virtual FName GetSmallIconName() const override;
    virtual FText GetDisplayName() const override;
    virtual FText GetDisplayTooltip() const override;
    virtual const FString& GetFilename() const override { return Filename; }
    virtual const FDateTime& GetTimeStamp() const override { return TimeStamp; }
    virtual bool CanCheckIn() const override { return bAdded || bCheckedOut || bModified || bDeleted; }
    virtual bool CanCheckout() const override { return bSourceControlled && !bCheckedOut && !bCheckedOutOther && bCurrent; }
    virtual bool IsCheckedOut() const override { return bCheckedOut; }
    virtual bool IsCheckedOutOther(FString* Who = nullptr) const override;
    virtual bool IsCheckedOutInOtherBranch(const FString&) const override { return false; }
    virtual bool IsCheckedOutOrModifiedInOtherBranch(const FString&) const override { return false; }
    virtual TArray<FString> GetCheckedOutBranches() const override { return {}; }
    virtual FString GetOtherUserBranchCheckedOuts() const override { return FString(); }
    virtual bool IsCurrent() const override { return bCurrent; }
    virtual bool IsSourceControlled() const override { return bSourceControlled; }
    virtual bool IsAdded() const override { return bAdded; }
    virtual bool IsDeleted() const override { return bDeleted; }
    virtual bool IsIgnored() const override { return false; }
    virtual bool CanEdit() const override { return !bCheckedOutOther; }
    virtual bool CanDelete() const override { return bSourceControlled && !bCheckedOutOther; }
    virtual bool IsUnknown() const override { return bUnknown; }
    virtual bool IsModified() const override { return bModified; }
    virtual bool IsModifiedInOtherBranch(const FString&) const override { return false; }
    virtual bool CanAdd() const override { return !bSourceControlled && !bDeleted; }
    virtual bool CanRevert() const override { return bAdded || bCheckedOut || bModified || bDeleted; }
    virtual bool IsConflicted() const override { return bCheckedOutOther || !bCurrent; }
    virtual bool IsLocal() const override { return bAdded; }
    virtual bool GetOtherBranchHeadModification(FString&, FString&, int32&) const override { return false; }

private:
    FString Filename;
    FString OtherUser;
    FDateTime TimeStamp;
    int32 LocalRevision = 0;
    int32 RemoteRevision = 0;
    bool bSourceControlled = false;
    bool bAdded = false;
    bool bDeleted = false;
    bool bCheckedOut = false;
    bool bCheckedOutOther = false;
    bool bCurrent = true;
    bool bModified = false;
    bool bUnknown = true;
};
