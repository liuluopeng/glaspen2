using System;
using System.Collections.Generic;
using System.Text;

namespace GlasPen2
{
    /// <summary>
    /// Minimal JSON serializer/deserializer for settings pipe protocol.
    /// Handles: strings, numbers, booleans, dictionaries, null.
    /// No external dependencies required.
    /// </summary>
    public static class SimpleJson
    {
        public static string Serialize(Dictionary<string, object> dict)
        {
            var sb = new StringBuilder();
            sb.Append('{');
            bool first = true;
            foreach (var kv in dict)
            {
                if (!first) sb.Append(',');
                first = false;
                sb.Append('"').Append(kv.Key).Append('"').Append(':');
                SerializeValue(sb, kv.Value);
            }
            sb.Append('}');
            return sb.ToString();
        }

        public static string SerializeObject(string type, Dictionary<string, object> data)
        {
            var sb = new StringBuilder();
            sb.Append('{');
            sb.Append("\"type\":\"").Append(type).Append('"');
            if (data != null)
            {
                sb.Append(",\"data\":");
                sb.Append(Serialize(data));
            }
            sb.Append('}');
            return sb.ToString();
        }

        private static void SerializeValue(StringBuilder sb, object value)
        {
            if (value == null)
            {
                sb.Append("null");
            }
            else if (value is bool)
            {
                sb.Append((bool)value ? "true" : "false");
            }
            else if (value is int || value is long || value is float || value is double)
            {
                sb.Append(value);
            }
            else if (value is string)
            {
                sb.Append('"').Append(EscapeString((string)value)).Append('"');
            }
            else
            {
                sb.Append('"').Append(EscapeString(value.ToString())).Append('"');
            }
        }

        private static string EscapeString(string s)
        {
            return s.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n").Replace("\r", "\\r");
        }

        public static Dictionary<string, object> Deserialize(string json)
        {
            var result = new Dictionary<string, object>();
            if (string.IsNullOrEmpty(json) || json[0] != '{') return result;

            int pos = 1; // skip '{'
            while (pos < json.Length && json[pos] != '}')
            {
                // Skip whitespace
                while (pos < json.Length && char.IsWhiteSpace(json[pos])) pos++;
                if (pos >= json.Length || json[pos] == '}') break;

                // Read key
                string key = ReadString(json, ref pos);
                if (string.IsNullOrEmpty(key)) break;

                // Skip ':'
                while (pos < json.Length && (json[pos] == ':' || char.IsWhiteSpace(json[pos]))) pos++;

                // Read value
                object value = ReadValue(json, ref pos);
                result[key] = value;

                // Skip comma
                while (pos < json.Length && (json[pos] == ',' || char.IsWhiteSpace(json[pos]))) pos++;
            }
            return result;
        }

        private static string ReadString(string json, ref int pos)
        {
            if (pos >= json.Length || json[pos] != '"') return "";
            pos++; // skip opening quote
            var sb = new StringBuilder();
            while (pos < json.Length && json[pos] != '"')
            {
                if (json[pos] == '\\' && pos + 1 < json.Length)
                {
                    pos++;
                    switch (json[pos])
                    {
                        case 'n': sb.Append('\n'); break;
                        case 'r': sb.Append('\r'); break;
                        case '\\': sb.Append('\\'); break;
                        case '"': sb.Append('"'); break;
                        default: sb.Append(json[pos]); break;
                    }
                }
                else
                {
                    sb.Append(json[pos]);
                }
                pos++;
            }
            if (pos < json.Length) pos++; // skip closing quote
            return sb.ToString();
        }

        private static object ReadValue(string json, ref int pos)
        {
            while (pos < json.Length && char.IsWhiteSpace(json[pos])) pos++;
            if (pos >= json.Length) return null;

            char c = json[pos];
            if (c == '"')
            {
                return ReadString(json, ref pos);
            }
            else if (c == 't' || c == 'f')
            {
                bool val = c == 't';
                pos += val ? 4 : 5;
                return val;
            }
            else if (c == 'n')
            {
                pos += 4;
                return null;
            }
            else if (c == '{')
            {
                // Nested object
                var nested = new Dictionary<string, object>();
                pos++; // skip '{'
                while (pos < json.Length && json[pos] != '}')
                {
                    while (pos < json.Length && char.IsWhiteSpace(json[pos])) pos++;
                    if (pos >= json.Length || json[pos] == '}') break;
                    string key = ReadString(json, ref pos);
                    while (pos < json.Length && (json[pos] == ':' || char.IsWhiteSpace(json[pos]))) pos++;
                    object value = ReadValue(json, ref pos);
                    nested[key] = value;
                    while (pos < json.Length && (json[pos] == ',' || char.IsWhiteSpace(json[pos]))) pos++;
                }
                if (pos < json.Length) pos++; // skip '}'
                return nested;
            }
            else
            {
                // Number
                int start = pos;
                while (pos < json.Length && (char.IsDigit(json[pos]) || json[pos] == '.' || json[pos] == '-')) pos++;
                string numStr = json.Substring(start, pos - start);
                if (numStr.Contains("."))
                {
                    double d;
                    double.TryParse(numStr, System.Globalization.NumberStyles.Float,
                        System.Globalization.CultureInfo.InvariantCulture, out d);
                    return d;
                }
                else
                {
                    int i;
                    int.TryParse(numStr, out i);
                    return i;
                }
            }
        }
    }
}
