using NUnit.Framework;

namespace OpenAssetDepot.Unity.Tests
{
    internal sealed class OpenAssetCliTests
    {
        [Test]
        public void QuoteArgumentPreservesSpacesAndQuotes()
        {
            Assert.AreEqual("\"Assets/Hero Scene.prefab\"", OpenAssetCli.QuoteArgument("Assets/Hero Scene.prefab"));
            Assert.AreEqual("\"say\\\"hello\"\"", OpenAssetCli.QuoteArgument("say\"hello\""));
        }

        [TestCase("Assets/Scene.unity", true)]
        [TestCase("Assets/Hero.prefab", true)]
        [TestCase("Assets/Code.cs", false)]
        [TestCase("Assets/Texture.png", false)]
        public void CheckoutRulesMatchUnityAssets(string path, bool expected)
        {
            Assert.AreEqual(expected, OpenAssetAssetHooks.RequiresCheckout(path));
        }
    }
}
