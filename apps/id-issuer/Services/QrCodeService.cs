using System.IO;
using System.Windows.Media.Imaging;
using QRCoder;

namespace IdIssuer.Services;

/// <summary>Renders text into a QR code <see cref="BitmapImage"/> for WPF.</summary>
public static class QrCodeService
{
    public static BitmapImage Create(string text, int pixelsPerModule = 8)
    {
        using var generator = new QRCodeGenerator();
        using var data = generator.CreateQrCode(text, QRCodeGenerator.ECCLevel.Q);
        byte[] png = new PngByteQRCode(data).GetGraphic(pixelsPerModule);

        var image = new BitmapImage();
        using var stream = new MemoryStream(png);
        image.BeginInit();
        image.CacheOption = BitmapCacheOption.OnLoad;
        image.StreamSource = stream;
        image.EndInit();
        image.Freeze(); // usable from any thread, immutable
        return image;
    }
}
