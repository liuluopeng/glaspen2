using System;
using System.Drawing;
using System.Windows.Forms;

namespace GlasPen2
{
    public class PressureForm : Form
    {
        private Label _label;
        private Timer _timer;

        // Shared state from OverlayForm
        public uint CurrentPressure;
        public bool TipDown;
        public bool InRange;
        public int ScreenX, ScreenY;

        public PressureForm()
        {
            this.FormBorderStyle = FormBorderStyle.None;
            this.ShowInTaskbar = false;
            this.TopMost = true;
            this.ShowIcon = false;
            this.StartPosition = FormStartPosition.Manual;
            this.Size = new Size(200, 30);
            this.Location = new Point(10, 10);
            this.BackColor = Color.FromArgb(30, 30, 30);
            this.DoubleBuffered = true;

            _label = new Label
            {
                Dock = DockStyle.Fill,
                ForeColor = Color.White,
                Font = new Font("Consolas", 12f, FontStyle.Bold),
                TextAlign = ContentAlignment.MiddleCenter,
                Text = "Pressure: ---"
            };
            this.Controls.Add(_label);

            _timer = new Timer { Interval = 50 }; // 20 FPS
            _timer.Tick += (s, e) => UpdateDisplay();
            _timer.Start();
        }

        private void UpdateDisplay()
        {
            string state = InRange ? (TipDown ? "DOWN" : "HOVER") : "AWAY";
            _label.Text = string.Format("P={0}  {1}  ({2},{3})", CurrentPressure, state, ScreenX, ScreenY);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            using (var pen = new Pen(Color.FromArgb(100, 255, 255, 255), 1f))
            {
                e.Graphics.DrawRectangle(pen, 0, 0, this.Width - 1, this.Height - 1);
            }
        }
    }
}
