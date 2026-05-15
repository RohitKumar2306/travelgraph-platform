{{- define "travelgraph.name" -}}
travelgraph
{{- end -}}

{{- define "travelgraph.labels" -}}
app.kubernetes.io/part-of: travelgraph
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version | replace "+" "_" }}
{{- end -}}

{{- define "travelgraph.image" -}}
{{- printf "%s/%s:%s" .root.Values.global.imageRegistry .image .root.Values.global.imageTag -}}
{{- end -}}

