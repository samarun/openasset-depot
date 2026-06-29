#pragma once

#include "HAL/ThreadSafeCounter.h"
#include "ISourceControlProvider.h"
#include "OpenAssetDepotState.h"

class FJsonObject;

class FOpenAssetDepotProvider final : public ISourceControlProvider
{
public:
    FOpenAssetDepotProvider();
    virtual ~FOpenAssetDepotProvider() override;

    using ISourceControlProvider::Execute;

    virtual void Init(bool bForceConnection = true) override;
    virtual void Close() override;
    virtual const FName& GetName() const override;
    virtual FText GetStatusText() const override;
    virtual TMap<EStatus, FString> GetStatus() const override;
    virtual bool IsEnabled() const override { return bEnabled; }
    virtual bool IsAvailable() const override { return bAvailable.Load(); }
    virtual bool UsesLocalReadOnlyState() const override { return false; }
    virtual bool UsesChangelists() const override { return false; }
    virtual bool UsesCheckout() const override { return true; }
    virtual bool UsesFileRevisions() const override { return true; }
    virtual bool UsesUncontrolledChangelists() const override { return false; }
    virtual bool UsesSnapshots() const override { return false; }
    virtual bool AllowsDiffAgainstDepot() const override { return false; }
    virtual TOptional<bool> IsAtLatestRevision() const override;
    virtual TOptional<int> GetNumLocalChanges() const override;
    virtual void Tick() override;
    virtual bool QueryStateBranchConfig(const FString&, const FString&) override { return false; }
    virtual void RegisterStateBranches(const TArray<FString>&, const FString&) override {}
    virtual int32 GetStateBranchIndex(const FString&) const override { return INDEX_NONE; }

    virtual ECommandResult::Type GetState(
        const TArray<FString>& InFiles,
        TArray<FSourceControlStateRef>& OutState,
        EStateCacheUsage::Type InStateCacheUsage) override;
    virtual ECommandResult::Type GetState(
        const TArray<FSourceControlChangelistRef>&,
        TArray<FSourceControlChangelistStateRef>& OutState,
        EStateCacheUsage::Type) override
    {
        OutState.Reset();
        return ECommandResult::Succeeded;
    }
    virtual TArray<FSourceControlStateRef> GetCachedStateByPredicate(
        TFunctionRef<bool(const FSourceControlStateRef&)> Predicate) const override;
    virtual TArray<TSharedRef<ISourceControlLabel>> GetLabels(const FString&) const override { return {}; }
    virtual TArray<FSourceControlChangelistRef> GetChangelists(EStateCacheUsage::Type) override { return {}; }

    virtual ECommandResult::Type Execute(
        const FSourceControlOperationRef& InOperation,
        FSourceControlChangelistPtr InChangelist,
        const TArray<FString>& InFiles,
        EConcurrency::Type InConcurrency,
        const FSourceControlOperationComplete& InOperationCompleteDelegate) override;
    virtual bool CanExecuteOperation(const FSourceControlOperationRef& InOperation) const override;
    virtual bool CanCancelOperation(const FSourceControlOperationRef&) const override { return false; }
    virtual void CancelOperation(const FSourceControlOperationRef&) override {}
#if SOURCE_CONTROL_WITH_SLATE
    virtual TSharedRef<SWidget> MakeSettingsWidget() const override;
#endif
    virtual FDelegateHandle RegisterSourceControlStateChanged_Handle(
        const FSourceControlStateChanged::FDelegate& SourceControlStateChanged) override;
    virtual void UnregisterSourceControlStateChanged_Handle(FDelegateHandle Handle) override;

private:
    ECommandResult::Type RunOperation(
        const FSourceControlOperationRef& Operation,
        const TArray<FString>& Files,
        FString& OutError);
    bool RefreshStates(const TArray<FString>& Files, FString& OutError);
    bool RunCli(const TArray<FString>& Arguments, TSharedPtr<FJsonObject>& OutEnvelope, FString& OutError) const;
    TSharedRef<FOpenAssetDepotState, ESPMode::ThreadSafe> GetOrCreateState(const FString& Filename);
    void BroadcastStateChanged();

    static const FName ProviderName;
    mutable FCriticalSection StateMutex;
    TMap<FString, TSharedRef<FOpenAssetDepotState, ESPMode::ThreadSafe>> StateCache;
    FSourceControlStateChanged StateChanged;
    FText StatusText;
    FThreadSafeCounter ActiveOperationCount;
    FEvent* OperationsIdleEvent = nullptr;
    TSharedPtr<int32, ESPMode::ThreadSafe> LifetimeToken;
    TAtomic<bool> bClosing{false};
    bool bEnabled = true;
    TAtomic<bool> bAvailable{false};
};
