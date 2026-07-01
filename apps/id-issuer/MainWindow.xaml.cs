using System.Windows;
using System.Windows.Media;
using IdIssuer.Services;

namespace IdIssuer;

public partial class MainWindow : Window
{
    private IdCard? _card;

    public MainWindow() => InitializeComponent();

    private void Issue_Click(object sender, RoutedEventArgs e)
    {
        var name = NameBox.Text.Trim();
        var id = IdBox.Text.Trim();
        if (name.Length == 0 || id.Length == 0)
        {
            SetStatus("請至少輸入姓名與身分證字號。", error: true);
            return;
        }

        var tag = TagBox.Text.Trim();
        if (tag.Length == 0) tag = "tw";

        _card = IdCard.Issue(name, id, DobBox.Text.Trim(), NatBox.Text.Trim(),
            tag, IssuerBox.Text.Trim());

        RenderCard(_card);
        SetStatus($"已核發：{_card.Did}");
    }

    private void Clear_Click(object sender, RoutedEventArgs e)
    {
        NameBox.Clear();
        IdBox.Clear();
        DobBox.Clear();
        NatBox.Text = "中華民國";
        TagBox.Text = "tw";
        IssuerBox.Text = "內政部";

        _card = null;
        PvName.Text = "—";
        PvId.Text = PvDob.Text = PvNat.Text = PvIssuer.Text = PvDate.Text = "—";
        PvDid.Text = PvHash.Text = "—";
        QrImage.Source = null;
        QrImage.Visibility = Visibility.Collapsed;
        QrPlaceholder.Visibility = Visibility.Visible;
        AnchorButton.IsEnabled = false;
        SetStatus("就緒 — 填寫左側資料後按「核發」。");
    }

    private async void Anchor_Click(object sender, RoutedEventArgs e)
    {
        if (_card is null)
        {
            SetStatus("請先核發身分證。", error: true);
            return;
        }
#if FERRUM_CHAIN
        var secret = SecretBox.Text.Trim();
        if (secret.Length == 0)
        {
            SetStatus("上鏈需要發證機關的 sr25519 secret seed（32-byte hex）。", error: true);
            return;
        }

        var endpoint = EndpointBox.Text.Trim();
        if (endpoint.Length == 0) endpoint = ChainService.DefaultEndpoint;

        AnchorButton.IsEnabled = false;
        SetStatus("連線並送出 anchor_did 交易中…");
        try
        {
            var hash = await ChainService.AnchorAsync(endpoint, secret, _card);
            SetStatus($"已上鏈，extrinsic：{hash}");
        }
        catch (Exception ex)
        {
            SetStatus("上鏈失敗：" + ex.Message, error: true);
        }
        finally
        {
            AnchorButton.IsEnabled = true;
        }
#else
        SetStatus("此版本未啟用上鏈功能，請以 -p:EnableChain=true 重新建置。", error: true);
        await Task.CompletedTask;
#endif
    }

    private void RenderCard(IdCard c)
    {
        PvName.Text = c.Name;
        PvId.Text = c.NationalId;
        PvDob.Text = Dash(c.BirthDate);
        PvNat.Text = Dash(c.Nationality);
        PvIssuer.Text = Dash(c.Issuer);
        PvDate.Text = c.IssuedAt;
        PvDid.Text = c.Did;
        PvHash.Text = c.DocHashHex;

        QrImage.Source = QrCodeService.Create(c.ToQrPayload());
        QrImage.Visibility = Visibility.Visible;
        QrPlaceholder.Visibility = Visibility.Collapsed;
        AnchorButton.IsEnabled = true;
    }

    private void SetStatus(string message, bool error = false)
    {
        StatusText.Text = message;
        StatusText.Foreground = error
            ? (Brush)FindResource("Danger")
            : (Brush)FindResource("Muted");
    }

    private static string Dash(string s) => string.IsNullOrWhiteSpace(s) ? "—" : s;
}
