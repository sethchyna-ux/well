part of 'generated.dart';

class ListMyCommandHistoryVariablesBuilder {
  
  final FirebaseDataConnect _dataConnect;
  ListMyCommandHistoryVariablesBuilder(this._dataConnect, );
  Deserializer<ListMyCommandHistoryData> dataDeserializer = (dynamic json)  => ListMyCommandHistoryData.fromJson(jsonDecode(json));
  
  Future<QueryResult<ListMyCommandHistoryData, void>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<ListMyCommandHistoryData, void> ref() {
    
    return _dataConnect.query("ListMyCommandHistory", dataDeserializer, emptySerializer, null);
  }
}

@immutable
class ListMyCommandHistoryCommandHistories {
  final String commandString;
  ListMyCommandHistoryCommandHistories.fromJson(dynamic json):
  
  commandString = nativeFromJson<String>(json['commandString']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyCommandHistoryCommandHistories otherTyped = other as ListMyCommandHistoryCommandHistories;
    return commandString == otherTyped.commandString;
    
  }
  @override
  int get hashCode => commandString.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['commandString'] = nativeToJson<String>(commandString);
    return json;
  }

  ListMyCommandHistoryCommandHistories({
    required this.commandString,
  });
}

@immutable
class ListMyCommandHistoryData {
  final List<ListMyCommandHistoryCommandHistories> commandHistories;
  ListMyCommandHistoryData.fromJson(dynamic json):
  
  commandHistories = (json['commandHistories'] as List<dynamic>)
        .map((e) => ListMyCommandHistoryCommandHistories.fromJson(e))
        .toList();
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyCommandHistoryData otherTyped = other as ListMyCommandHistoryData;
    return commandHistories == otherTyped.commandHistories;
    
  }
  @override
  int get hashCode => commandHistories.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['commandHistories'] = commandHistories.map((e) => e.toJson()).toList();
    return json;
  }

  ListMyCommandHistoryData({
    required this.commandHistories,
  });
}

