var OpenAssetDepot = OpenAssetDepot || {};

OpenAssetDepot.currentDocumentPath = function () {
    try {
        var host = BridgeTalk.appName;
        if (host === "photoshop" || host === "illustrator" || host === "indesign") {
            if (app.documents.length === 0) return "";
            return app.activeDocument.fullName.fsName;
        }
        if (host === "aftereffects") {
            if (!app.project || !app.project.file) return "";
            return app.project.file.fsName;
        }
        if (host === "premierepro") {
            return app.project && app.project.path ? app.project.path : "";
        }
    } catch (error) {
        return "";
    }
    return "";
};

OpenAssetDepot.hostName = function () {
    return BridgeTalk.appName || "Adobe Creative Cloud";
};
