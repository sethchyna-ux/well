part of 'generated.dart';

class ListMyShellConfigsVariablesBuilder {
  
  final FirebaseDataConnect _dataConnect;
  ListMyShellConfigsVariablesBuilder(this._dataConnect, );
  Deserializer<ListMyShellConfigsData> dataDeserializer = (dynamic json)  => ListMyShellConfigsData.fromJson(jsonDecode(json));
  
  Future<QueryResult<ListMyShellConfigsData, void>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<ListMyShellConfigsData, void> ref() {
    
    return _dataConnect.query("ListMyShellConfigs", dataDeserializer, emptySerializer, null);
  }
}

@immutable
class ListMyShellConfigsShellConfigs {
  final String promptTemplate;
  ListMyShellConfigsShellConfigs.fromJson(dynamic json):
  
  promptTemplate = nativeFromJson<String>(json['promptTemplate']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyShellConfigsShellConfigs otherTyped = other as ListMyShellConfigsShellConfigs;
    return promptTemplate == otherTyped.promptTemplate;
    
  }
  @override
  int get hashCode => promptTemplate.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['promptTemplate'] = nativeToJson<String>(promptTemplate);
    return json;
  }

  ListMyShellConfigsShellConfigs({
    required this.promptTemplate,
  });
}

@immutable
class ListMyShellConfigsData {
  final List<ListMyShellConfigsShellConfigs> shellConfigs;
  ListMyShellConfigsData.fromJson(dynamic json):
  
  shellConfigs = (json['shellConfigs'] as List<dynamic>)
        .map((e) => ListMyShellConfigsShellConfigs.fromJson(e))
        .toList();
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyShellConfigsData otherTyped = other as ListMyShellConfigsData;
    return shellConfigs == otherTyped.shellConfigs;
    
  }
  @override
  int get hashCode => shellConfigs.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['shellConfigs'] = shellConfigs.map((e) => e.toJson()).toList();
    return json;
  }

  ListMyShellConfigsData({
    required this.shellConfigs,
  });
}

