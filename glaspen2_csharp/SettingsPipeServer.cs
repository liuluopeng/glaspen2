using System;
using System.Collections.Generic;
using System.IO.Pipes;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace GlasPen2
{
    /// <summary>
    /// Named pipe server for Flutter settings window communication.
    /// Protocol: newline-delimited JSON messages.
    /// </summary>
    public class SettingsPipeServer : IDisposable
    {
        private const string PipeName = "glaspen2_settings";
        private CancellationTokenSource _cts;
        private List<NamedPipeServerStream> _clients = new List<NamedPipeServerStream>();
        private readonly object _lock = new object();

        public Func<Dictionary<string, object>> GetSettings { get; set; }
        public Action<string, object> OnSettingChanged { get; set; }

        public void Start()
        {
            _cts = new CancellationTokenSource();
            CancellationToken ct = _cts.Token;
            Task.Run(() => AcceptClients(ct));
            Console.WriteLine("[Pipe] Settings pipe server started: \\\\.\\pipe\\{0}", PipeName);
        }

        public void Stop()
        {
            if (_cts != null) _cts.Cancel();
            lock (_lock)
            {
                foreach (var client in _clients)
                {
                    try { client.Dispose(); } catch { }
                }
                _clients.Clear();
            }
        }

        public void NotifySettingsChanged(Dictionary<string, object> settings)
        {
            var msg = SimpleJson.SerializeObject("onSettingsChanged", settings);
            Broadcast(msg + "\n");
        }

        private void AcceptClients(CancellationToken ct)
        {
            while (!ct.IsCancellationRequested)
            {
                try
                {
                    var pipe = new NamedPipeServerStream(
                        PipeName,
                        PipeDirection.InOut,
                        NamedPipeServerStream.MaxAllowedServerInstances,
                        PipeTransmissionMode.Byte,
                        PipeOptions.Asynchronous);

                    pipe.WaitForConnection();
                    Console.WriteLine("[Pipe] Flutter client connected");

                    lock (_lock) { _clients.Add(pipe); }

                    var clientCt = ct;
                    Task.Run(() => HandleClient(pipe, clientCt));
                }
                catch (OperationCanceledException) { break; }
                catch (Exception e)
                {
                    Console.WriteLine("[Pipe] Accept error: {0}", e.Message);
                    Thread.Sleep(1000);
                }
            }
        }

        private void HandleClient(NamedPipeServerStream pipe, CancellationToken ct)
        {
            try
            {
                var buffer = new byte[4096];
                var lineBuffer = new StringBuilder();

                while (pipe.IsConnected && !ct.IsCancellationRequested)
                {
                    int bytesRead = pipe.Read(buffer, 0, buffer.Length);
                    if (bytesRead == 0) break;

                    for (int i = 0; i < bytesRead; i++)
                    {
                        if (buffer[i] == (byte)'\n')
                        {
                            string line = lineBuffer.ToString();
                            lineBuffer.Clear();
                            if (!string.IsNullOrEmpty(line))
                            {
                                ProcessMessage(pipe, line);
                            }
                        }
                        else
                        {
                            lineBuffer.Append((char)buffer[i]);
                        }
                    }
                }
            }
            catch (OperationCanceledException) { }
            catch (Exception e)
            {
                Console.WriteLine("[Pipe] Client error: {0}", e.Message);
            }
            finally
            {
                lock (_lock) { _clients.Remove(pipe); }
                try { pipe.Dispose(); } catch { }
                Console.WriteLine("[Pipe] Flutter client disconnected");
            }
        }

        private void ProcessMessage(NamedPipeServerStream pipe, string line)
        {
            try
            {
                var msg = SimpleJson.Deserialize(line);
                if (msg.Count == 0 || !msg.ContainsKey("type")) return;

                string type = msg["type"].ToString();

                if (type == "getSettings")
                {
                    var settings = GetSettings != null ? GetSettings.Invoke() : new Dictionary<string, object>();
                    var response = SimpleJson.SerializeObject("getSettings_response", settings);
                    WriteToClient(pipe, response + "\n");
                }
                else if (type == "setSetting")
                {
                    string key = msg.ContainsKey("key") ? msg["key"].ToString() : "";
                    object value = msg.ContainsKey("value") ? msg["value"] : null;
                    if (!string.IsNullOrEmpty(key) && OnSettingChanged != null)
                    {
                        OnSettingChanged.Invoke(key, value);
                    }
                }
            }
            catch (Exception e)
            {
                Console.WriteLine("[Pipe] Message parse error: {0}", e.Message);
            }
        }

        private void WriteToClient(NamedPipeServerStream pipe, string data)
        {
            try
            {
                byte[] bytes = Encoding.UTF8.GetBytes(data);
                pipe.Write(bytes, 0, bytes.Length);
                pipe.Flush();
            }
            catch (Exception e)
            {
                Console.WriteLine("[Pipe] Write error: {0}", e.Message);
            }
        }

        private void Broadcast(string data)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(data);
            List<NamedPipeServerStream> snapshot;
            lock (_lock) { snapshot = new List<NamedPipeServerStream>(_clients); }

            foreach (var pipe in snapshot)
            {
                try
                {
                    if (pipe.IsConnected)
                    {
                        pipe.Write(bytes, 0, bytes.Length);
                        pipe.Flush();
                    }
                }
                catch { }
            }
        }

        public void Dispose()
        {
            Stop();
        }
    }
}
