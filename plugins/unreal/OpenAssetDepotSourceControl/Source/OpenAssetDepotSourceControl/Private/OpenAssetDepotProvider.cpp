#include "OpenAssetDepotProvider.h"

#include "Async/Async.h"
#include "Dom/JsonObject.h"
#include "HAL/Event.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Interfaces/IPluginManager.h"
#include "Misc/Paths.h"
#include "Misc/ScopeLock.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "SourceControlOperations.h"
#include "Widgets/Layout/SBorder.h"
#include "Widgets/Layout/SVerticalBox.h"
#include "Widgets/Text/STextBlock.h"

const FName FOpenAssetDepotProvider::ProviderName(TEXT("OpenAsset Depot"));

namespace
{
FString QuoteArgument(const FString& Value)
{
    FString Escaped = Value.Replace(TEXT("\""), TEXT("\\\""));
    return FString::Printf(TEXT("\"%s\""), *Escaped);
}
}

FOpenAssetDepotProvider::FOpenAssetDepotProvider()
    : StatusText(NSLOCTEXT("OpenAssetDepot", "Disconnected", "OpenAsset Depot is not connected"))
{
    OperationsIdleEvent = FPlatformProcess::GetSynchEventFromPool(true);
    OperationsIdleEvent->Trigger();
    LifetimeToken = MakeShared<int32, ESPMode::ThreadSafe>(0);
}

FOpenAssetDepotProvider::~FOpenAssetDepotProvider()
{
    Close();
    FPlatformProcess::ReturnSynchEventToPool(OperationsIdleEvent);
    OperationsIdleEvent = nullptr;
}

void FOpenAssetDepotProvider::Init(bool)
{
    bClosing.Store(false);
    if (!LifetimeToken.IsValid()) LifetimeToken = MakeShared<int32, ESPMode::ThreadSafe>(0);
    TSharedPtr<FJsonObject> Envelope;
    FString Error;
    const bool bConnected = RunCli({TEXT("integration"), TEXT("context")}, Envelope, Error);
    bAvailable.Store(bConnected);
    StatusText = bConnected
        ? NSLOCTEXT("OpenAssetDepot", "Connected", "OpenAsset Depot workspace is ready")
        : FText::FromString(Error);
}

void FOpenAssetDepotProvider::Close()
{
    bClosing.Store(true);
    LifetimeToken.Reset();
    if (ActiveOperationCount.GetValue() > 0 && OperationsIdleEvent)
    {
        OperationsIdleEvent->Wait();
    }
    bAvailable.Store(false);
    FScopeLock Lock(&StateMutex);
    StateCache.Empty();
}

const FName& FOpenAssetDepotProvider::GetName() const
{
    return ProviderName;
}

FText FOpenAssetDepotProvider::GetStatusText() const
{
    return StatusText;
}

TMap<ISourceControlProvider::EStatus, FString> FOpenAssetDepotProvider::GetStatus() const
{
    TMap<EStatus, FString> Result;
    Result.Add(EStatus::Enabled, bEnabled ? TEXT("Yes") : TEXT("No"));
    Result.Add(EStatus::Connected, bAvailable.Load() ? TEXT("Yes") : TEXT("No"));
    return Result;
}

TOptional<bool> FOpenAssetDepotProvider::IsAtLatestRevision() const
{
    FScopeLock Lock(&StateMutex);
    for (const auto& Pair : StateCache)
    {
        if (!Pair.Value->IsCurrent()) return false;
    }
    return true;
}

TOptional<int> FOpenAssetDepotProvider::GetNumLocalChanges() const
{
    int Count = 0;
    FScopeLock Lock(&StateMutex);
    for (const auto& Pair : StateCache)
    {
        if (Pair.Value->CanCheckIn()) ++Count;
    }
    return Count;
}

void FOpenAssetDepotProvider::Tick()
{
}

ECommandResult::Type FOpenAssetDepotProvider::GetState(
    const TArray<FString>& InFiles,
    TArray<FSourceControlStateRef>& OutState,
    EStateCacheUsage::Type InStateCacheUsage)
{
    if (InStateCacheUsage == EStateCacheUsage::ForceUpdate)
    {
        FString Error;
        if (!RefreshStates(InFiles, Error))
        {
            StatusText = FText::FromString(Error);
            return ECommandResult::Failed;
        }
    }
    for (const FString& File : InFiles)
    {
        OutState.Add(GetOrCreateState(FPaths::ConvertRelativePathToFull(File)));
    }
    return ECommandResult::Succeeded;
}

TArray<FSourceControlStateRef> FOpenAssetDepotProvider::GetCachedStateByPredicate(
    TFunctionRef<bool(const FSourceControlStateRef&)> Predicate) const
{
    TArray<FSourceControlStateRef> Result;
    FScopeLock Lock(&StateMutex);
    for (const auto& Pair : StateCache)
    {
        if (Predicate(Pair.Value)) Result.Add(Pair.Value);
    }
    return Result;
}

ECommandResult::Type FOpenAssetDepotProvider::Execute(
    const FSourceControlOperationRef& InOperation,
    FSourceControlChangelistPtr,
    const TArray<FString>& InFiles,
    EConcurrency::Type InConcurrency,
    const FSourceControlOperationComplete& InOperationCompleteDelegate)
{
    if (InConcurrency == EConcurrency::Asynchronous)
    {
        if (bClosing.Load()) return ECommandResult::Failed;
        const TWeakPtr<int32, ESPMode::ThreadSafe> WeakLifetime = LifetimeToken;
        OperationsIdleEvent->Reset();
        ActiveOperationCount.Increment();
        Async(EAsyncExecution::ThreadPool, [this, WeakLifetime, InOperation, InFiles, InOperationCompleteDelegate]()
        {
            FString Error;
            const ECommandResult::Type Result = RunOperation(InOperation, InFiles, Error);
            AsyncTask(ENamedThreads::GameThread, [this, WeakLifetime, InOperation, InOperationCompleteDelegate, Result, Error]()
            {
                if (!WeakLifetime.IsValid()) return;
                bAvailable.Store(Result == ECommandResult::Succeeded);
                StatusText = Error.IsEmpty()
                    ? NSLOCTEXT("OpenAssetDepot", "Ready", "OpenAsset Depot workspace is ready")
                    : FText::FromString(Error);
                BroadcastStateChanged();
                InOperationCompleteDelegate.ExecuteIfBound(InOperation, Result);
            });
            if (ActiveOperationCount.Decrement() == 0) OperationsIdleEvent->Trigger();
        });
        return ECommandResult::Succeeded;
    }

    FString Error;
    const ECommandResult::Type Result = RunOperation(InOperation, InFiles, Error);
    bAvailable.Store(Result == ECommandResult::Succeeded);
    StatusText = Error.IsEmpty()
        ? NSLOCTEXT("OpenAssetDepot", "Ready", "OpenAsset Depot workspace is ready")
        : FText::FromString(Error);
    BroadcastStateChanged();
    InOperationCompleteDelegate.ExecuteIfBound(InOperation, Result);
    return Result;
}

bool FOpenAssetDepotProvider::CanExecuteOperation(const FSourceControlOperationRef& InOperation) const
{
    const FName Name = InOperation->GetName();
    return Name == FConnect::StaticName()
        || Name == FUpdateStatus::StaticName()
        || Name == FCheckOut::StaticName()
        || Name == FMarkForAdd::StaticName()
        || Name == FDelete::StaticName()
        || Name == FRevert::StaticName()
        || Name == FSync::StaticName()
        || Name == FCheckIn::StaticName();
}

#if SOURCE_CONTROL_WITH_SLATE
TSharedRef<SWidget> FOpenAssetDepotProvider::MakeSettingsWidget() const
{
    return SNew(SBorder)
        .Padding(12)
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot().AutoHeight()
            [
                SNew(STextBlock).Text(NSLOCTEXT(
                    "OpenAssetDepot",
                    "SettingsHelp",
                    "Use the OpenAsset desktop app to create a workspace, then run 'oad login'. Set OAD_CLI if the CLI is not on PATH."))
                .AutoWrapText(true)
            ]
        ];
}
#endif

FDelegateHandle FOpenAssetDepotProvider::RegisterSourceControlStateChanged_Handle(
    const FSourceControlStateChanged::FDelegate& Delegate)
{
    return StateChanged.Add(Delegate);
}

void FOpenAssetDepotProvider::UnregisterSourceControlStateChanged_Handle(FDelegateHandle Handle)
{
    StateChanged.Remove(Handle);
}

ECommandResult::Type FOpenAssetDepotProvider::RunOperation(
    const FSourceControlOperationRef& Operation,
    const TArray<FString>& Files,
    FString& OutError)
{
    const FName Name = Operation->GetName();
    if (Name == FUpdateStatus::StaticName())
    {
        return RefreshStates(Files, OutError) ? ECommandResult::Succeeded : ECommandResult::Failed;
    }

    TArray<FString> Arguments{TEXT("integration")};
    if (Name == FConnect::StaticName())
    {
        Arguments.Add(TEXT("context"));
    }
    else if (Name == FSync::StaticName())
    {
        Arguments.Add(TEXT("sync"));
    }
    else if (Name == FCheckIn::StaticName())
    {
        Arguments.Add(TEXT("submit"));
        Arguments.Add(TEXT("--description"));
        Arguments.Add(StaticCastSharedRef<FCheckIn>(Operation)->GetDescription().ToString());
    }
    else
    {
        FString Verb;
        if (Name == FCheckOut::StaticName()) Verb = TEXT("checkout");
        else if (Name == FMarkForAdd::StaticName()) Verb = TEXT("add");
        else if (Name == FDelete::StaticName()) Verb = TEXT("delete");
        else if (Name == FRevert::StaticName()) Verb = TEXT("revert");
        else return ECommandResult::Failed;

        for (const FString& File : Files)
        {
            TSharedPtr<FJsonObject> Envelope;
            TArray<FString> FileArguments{TEXT("integration"), Verb, FPaths::ConvertRelativePathToFull(File)};
            if (!RunCli(FileArguments, Envelope, OutError)) return ECommandResult::Failed;
        }
        RefreshStates(Files, OutError);
        return ECommandResult::Succeeded;
    }

    TSharedPtr<FJsonObject> Envelope;
    const bool bSuccess = RunCli(Arguments, Envelope, OutError);
    if (bSuccess && Files.Num() > 0) RefreshStates(Files, OutError);
    return bSuccess ? ECommandResult::Succeeded : ECommandResult::Failed;
}

bool FOpenAssetDepotProvider::RefreshStates(const TArray<FString>& Files, FString& OutError)
{
    if (Files.IsEmpty()) return true;
    TArray<FString> Arguments{TEXT("integration"), TEXT("status")};
    for (const FString& File : Files) Arguments.Add(FPaths::ConvertRelativePathToFull(File));
    TSharedPtr<FJsonObject> Envelope;
    if (!RunCli(Arguments, Envelope, OutError)) return false;
    const TSharedPtr<FJsonObject>* Data = nullptr;
    if (!Envelope->TryGetObjectField(TEXT("data"), Data) || !Data || !Data->IsValid())
    {
        OutError = TEXT("OpenAsset CLI returned no status data");
        return false;
    }
    const TArray<TSharedPtr<FJsonValue>>* JsonFiles = nullptr;
    if (!(*Data)->TryGetArrayField(TEXT("files"), JsonFiles) || !JsonFiles)
    {
        OutError = TEXT("OpenAsset CLI returned an invalid file status list");
        return false;
    }
    for (const TSharedPtr<FJsonValue>& Value : *JsonFiles)
    {
        const TSharedPtr<FJsonObject> Json = Value->AsObject();
        if (!Json) continue;
        FString DepotPath;
        Json->TryGetStringField(TEXT("path"), DepotPath);
        FString Absolute;
        if (!Json->TryGetStringField(TEXT("local_path"), Absolute) || Absolute.IsEmpty())
        {
            Absolute = FPaths::ConvertRelativePathToFull(FPaths::ProjectDir(), DepotPath);
        }
        GetOrCreateState(Absolute)->UpdateFromJson(Json);
    }
    return true;
}

bool FOpenAssetDepotProvider::RunCli(
    const TArray<FString>& Arguments,
    TSharedPtr<FJsonObject>& OutEnvelope,
    FString& OutError) const
{
    FString CliPath = FPlatformMisc::GetEnvironmentVariable(TEXT("OAD_CLI"));
    if (CliPath.IsEmpty()) CliPath = TEXT("oad");
    TArray<FString> FullArguments{TEXT("--cwd"), FPaths::ConvertRelativePathToFull(FPaths::ProjectDir())};
    FullArguments.Append(Arguments);
    FString CommandLine;
    for (const FString& Argument : FullArguments)
    {
        if (!CommandLine.IsEmpty()) CommandLine.AppendChar(TEXT(' '));
        CommandLine.Append(QuoteArgument(Argument));
    }
    int32 ReturnCode = -1;
    FString StandardOutput;
    FString StandardError;
    void* StandardOutputRead = nullptr;
    void* StandardOutputWrite = nullptr;
    void* StandardErrorRead = nullptr;
    void* StandardErrorWrite = nullptr;
    if (!FPlatformProcess::CreatePipe(StandardOutputRead, StandardOutputWrite)
        || !FPlatformProcess::CreatePipe(StandardErrorRead, StandardErrorWrite))
    {
        if (StandardOutputRead || StandardOutputWrite) FPlatformProcess::ClosePipe(StandardOutputRead, StandardOutputWrite);
        if (StandardErrorRead || StandardErrorWrite) FPlatformProcess::ClosePipe(StandardErrorRead, StandardErrorWrite);
        OutError = TEXT("Could not create pipes for the OpenAsset CLI");
        return false;
    }
    FProcHandle Process = FPlatformProcess::CreateProc(
        *CliPath,
        *CommandLine,
        false,
        true,
        true,
        nullptr,
        0,
        *FPaths::ConvertRelativePathToFull(FPaths::ProjectDir()),
        StandardOutputWrite,
        nullptr,
        StandardErrorWrite);
    if (!Process.IsValid())
    {
        FPlatformProcess::ClosePipe(StandardOutputRead, StandardOutputWrite);
        FPlatformProcess::ClosePipe(StandardErrorRead, StandardErrorWrite);
        OutError = FString::Printf(TEXT("Could not start OpenAsset CLI at '%s'"), *CliPath);
        return false;
    }

    const bool bLongOperation = Arguments.Contains(TEXT("sync")) || Arguments.Contains(TEXT("submit"));
    const double Deadline = FPlatformTime::Seconds() + (bLongOperation ? 30.0 * 60.0 : 120.0);
    bool bTimedOut = false;
    bool bOutputExceeded = false;
    while (FPlatformProcess::IsProcRunning(Process))
    {
        StandardOutput += FPlatformProcess::ReadPipe(StandardOutputRead);
        StandardError += FPlatformProcess::ReadPipe(StandardErrorRead);
        bOutputExceeded = StandardOutput.Len() > 4 * 1024 * 1024 || StandardError.Len() > 4 * 1024 * 1024;
        bTimedOut = FPlatformTime::Seconds() >= Deadline;
        if (bClosing.Load() || bTimedOut || bOutputExceeded)
        {
            FPlatformProcess::TerminateProc(Process, true);
            break;
        }
        FPlatformProcess::Sleep(0.01f);
    }
    FPlatformProcess::WaitForProc(Process);
    StandardOutput += FPlatformProcess::ReadPipe(StandardOutputRead);
    StandardError += FPlatformProcess::ReadPipe(StandardErrorRead);
    FPlatformProcess::GetProcReturnCode(Process, &ReturnCode);
    FPlatformProcess::CloseProc(Process);
    FPlatformProcess::ClosePipe(StandardOutputRead, StandardOutputWrite);
    FPlatformProcess::ClosePipe(StandardErrorRead, StandardErrorWrite);

    if (bClosing.Load())
    {
        OutError = TEXT("OpenAsset operation was cancelled because the provider is closing");
        return false;
    }
    if (bTimedOut)
    {
        OutError = TEXT("OpenAsset CLI operation timed out");
        return false;
    }
    if (bOutputExceeded || StandardOutput.Len() > 4 * 1024 * 1024 || StandardError.Len() > 4 * 1024 * 1024)
    {
        OutError = TEXT("OpenAsset CLI output exceeded the 4 MiB safety limit");
        return false;
    }
    const TSharedRef<TJsonReader<>> Reader = TJsonReaderFactory<>::Create(StandardOutput);
    if (!FJsonSerializer::Deserialize(Reader, OutEnvelope) || !OutEnvelope)
    {
        OutError = StandardError.IsEmpty() ? TEXT("OpenAsset CLI returned invalid JSON") : StandardError;
        return false;
    }
    double ProtocolVersion = 0;
    if (!OutEnvelope->TryGetNumberField(TEXT("protocol_version"), ProtocolVersion) || ProtocolVersion != 1)
    {
        OutError = TEXT("OpenAsset CLI returned an unsupported integration protocol version");
        return false;
    }
    bool bOk = false;
    OutEnvelope->TryGetBoolField(TEXT("ok"), bOk);
    if (ReturnCode != 0 || !bOk)
    {
        if (!OutEnvelope->TryGetStringField(TEXT("error"), OutError)) OutError = StandardError;
        return false;
    }
    return true;
}

TSharedRef<FOpenAssetDepotState, ESPMode::ThreadSafe> FOpenAssetDepotProvider::GetOrCreateState(
    const FString& Filename)
{
    FScopeLock Lock(&StateMutex);
    if (const TSharedRef<FOpenAssetDepotState, ESPMode::ThreadSafe>* Existing = StateCache.Find(Filename))
    {
        return *Existing;
    }
    TSharedRef<FOpenAssetDepotState, ESPMode::ThreadSafe> State = MakeShared<FOpenAssetDepotState, ESPMode::ThreadSafe>(Filename);
    StateCache.Add(Filename, State);
    return State;
}

void FOpenAssetDepotProvider::BroadcastStateChanged()
{
    StateChanged.Broadcast();
}
