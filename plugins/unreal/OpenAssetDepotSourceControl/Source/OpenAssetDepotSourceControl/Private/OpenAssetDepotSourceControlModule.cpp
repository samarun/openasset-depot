#include "OpenAssetDepotSourceControlModule.h"

#include "Features/IModularFeatures.h"
#include "ISourceControlModule.h"
#include "OpenAssetDepotProvider.h"

void FOpenAssetDepotSourceControlModule::StartupModule()
{
    Provider = MakeUnique<FOpenAssetDepotProvider>();
    IModularFeatures::Get().RegisterModularFeature(TEXT("SourceControl"), Provider.Get());
}

void FOpenAssetDepotSourceControlModule::ShutdownModule()
{
    if (Provider)
    {
        Provider->Close();
        if (IModularFeatures::Get().IsModularFeatureAvailable(TEXT("SourceControl")))
        {
            IModularFeatures::Get().UnregisterModularFeature(TEXT("SourceControl"), Provider.Get());
        }
        Provider.Reset();
    }
}

FOpenAssetDepotProvider& FOpenAssetDepotSourceControlModule::GetProvider() const
{
    check(Provider);
    return *Provider;
}

IMPLEMENT_MODULE(FOpenAssetDepotSourceControlModule, OpenAssetDepotSourceControl)
