using System;
using System.Windows.Forms;

namespace GlasPen2
{
    /// <summary>Minimal window below overlay. Fully transparent to input + visually.</summary>
    public class InputWindow : Form
    {
        public InputWindow()
        {
            var bounds = SystemInformation.VirtualScreen;
            this.StartPosition = FormStartPosition.Manual;
            this.Location = bounds.Location;
            this.Size = bounds.Size;
            this.FormBorderStyle = FormBorderStyle.None;
            this.ShowInTaskbar = false;
            this.ShowIcon = false;
            this.BackColor = System.Drawing.Color.Fuchsia;
            this.TransparencyKey = System.Drawing.Color.Fuchsia;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            NativeMethods.SetWindowPos(this.Handle, new IntPtr(-2),
                this.Left, this.Top, this.Width, this.Height,
                NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == 0x0084) { m.Result = new IntPtr(-1); return; }
            base.WndProc(ref m);
        }

        protected override bool ShowWithoutActivation { get { return true; } }
        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                cp.ExStyle |= NativeMethods.WS_EX_TRANSPARENT
                           | NativeMethods.WS_EX_NOACTIVATE
                           | NativeMethods.WS_EX_TOOLWINDOW;
                return cp;
            }
        }
    }
}
