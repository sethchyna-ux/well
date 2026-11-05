part of 'generated.dart';

class ListMyTerminalThemesVariablesBuilder {
  
  final FirebaseDataConnect _dataConnect;
  ListMyTerminalThemesVariablesBuilder(this._dataConnect, );
  Deserializer<ListMyTerminalThemesData> dataDeserializer = (dynamic json)  => ListMyTerminalThemesData.fromJson(jsonDecode(json));
  
  Future<QueryResult<ListMyTerminalThemesData, void>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<ListMyTerminalThemesData, void> ref() {
    
    return _dataConnect.query("ListMyTerminalThemes", dataDeserializer, emptySerializer, null);
  }
}

@immutable
class ListMyTerminalThemesTerminalThemes {
  final String name;
  ListMyTerminalThemesTerminalThemes.fromJson(dynamic json):
  
  name = nativeFromJson<String>(json['name']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyTerminalThemesTerminalThemes otherTyped = other as ListMyTerminalThemesTerminalThemes;
    return name == otherTyped.name;
    
  }
  @override
  int get hashCode => name.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    return json;
  }

  ListMyTerminalThemesTerminalThemes({
    required this.name,
  });
}

@immutable
class ListMyTerminalThemesData {
  final List<ListMyTerminalThemesTerminalThemes> terminalThemes;
  ListMyTerminalThemesData.fromJson(dynamic json):
  
  terminalThemes = (json['terminalThemes'] as List<dynamic>)
        .map((e) => ListMyTerminalThemesTerminalThemes.fromJson(e))
        .toList();
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyTerminalThemesData otherTyped = other as ListMyTerminalThemesData;
    return terminalThemes == otherTyped.terminalThemes;
    
  }
  @override
  int get hashCode => terminalThemes.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['terminalThemes'] = terminalThemes.map((e) => e.toJson()).toList();
    return json;
  }

  ListMyTerminalThemesData({
    required this.terminalThemes,
  });
}

