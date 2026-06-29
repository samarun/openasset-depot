#pragma once

#include "Modules/ModuleManager.h"

class FOpenAssetDepotProvider;

class FOpenAssetDepotSourceControlModule final : public IModuleInterface
{
public:
    virtual void StartupModule() override;
    virtual void ShutdownModule() override;

    FOpenAssetDepotProvider& GetProvider() const;

private:
    TUniquePtr<FOpenAssetDepotProvider> Provider;
};
