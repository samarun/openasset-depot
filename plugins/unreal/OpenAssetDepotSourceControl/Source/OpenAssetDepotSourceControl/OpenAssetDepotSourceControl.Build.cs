using UnrealBuildTool;

public class OpenAssetDepotSourceControl : ModuleRules
{
    public OpenAssetDepotSourceControl(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PrivateDependencyModuleNames.AddRange(new[]
        {
            "Core",
            "CoreUObject",
            "Json",
            "Projects",
            "Slate",
            "SlateCore",
            "SourceControl"
        });
    }
}
